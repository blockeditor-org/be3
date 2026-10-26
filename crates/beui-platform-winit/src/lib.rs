#![cfg(not(any(target_os = "android", target_arch = "wasm32")))]

mod accessibility;
mod clipboard;
mod file_picker;
mod file_saver;
mod preferences;
#[cfg(test)]
mod tests;
mod wheel;

pub use winit;

use std::cell::RefCell;
use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::cursor::{CustomCursor, CustomCursorSource};
use winit::data_transfer::TypeHint;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{
    ButtonSource, DeviceEvent, DeviceId, ElementState, FingerId, Force, Ime, MouseButton,
    MouseScrollDelta, PointerKind, PointerSource, TabletToolButton, TabletToolData, TabletToolKind,
    WindowEvent,
};
use winit::event_loop::{
    ActiveEventLoop, AsyncRequestSerial, ControlFlow, DndAction, EventLoop, EventLoopProxy,
};
use winit::keyboard::{NativeKeyCode, PhysicalKey};
use winit::monitor::Fullscreen;
use winit::window::{
    CursorGrabMode, ImeCapabilities, ImeEnableRequest, ImeRequest, ImeRequestData, Window,
    WindowAttributes, WindowId,
};

use accessibility::{Accessibility, Request};
use accesskit::TreeUpdate;
use beui_core::app::{Lifecycle, SafeArea, Setup, Waker};
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{
    CursorIcon, DroppedFile, FileDrag, ImeArea, ImeEvent, Input, Key, Keystroke, Modifiers, Pen,
    PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
use beui_core::renderer::{Loaded, WindowHandle};
use beui_core::runner::{Incoming, Launch, Platform, PlatformServices, Running};
use beui_core::window::WindowState;
use clipboard::Clipboard;
use preferences::SystemPreferences;
use wheel::{Wheel, Wheeled};

const BUTTON_EVDEV_FIRST: u16 = 0x110;
const XKB_AUDIO_MIC_MUTE: u32 = 0x1008_ffb2;

type Load = Box<dyn FnOnce(Arc<dyn WindowHandle>) -> Result<Loaded, Box<dyn Error>>>;

type Outcome = Rc<RefCell<Option<Result<(), String>>>>;

pub struct Winit {
    load: Load,
}

impl Winit {
    pub fn new(
        load: impl FnOnce(Arc<dyn WindowHandle>) -> Result<Loaded, Box<dyn Error>> + 'static,
    ) -> Self {
        Self {
            load: Box::new(load),
        }
    }
}

impl Platform for Winit {
    fn name(&self) -> &'static str {
        "winit"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(launch, self.load) })
    }
}

fn run(launch: Launch, load: Load) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let size = launch.options.size;
    #[cfg(all(unix, not(target_os = "macos")))]
    let app_id = launch.options.app_id.clone();
    let outcome = Outcome::default();
    let runner = Runner {
        runner: beui_core::runner::Runner::new(launch),
        size,
        #[cfg(all(unix, not(target_os = "macos")))]
        app_id,
        load: Some(load),
        surface: None,
        pointer: Pos2::ZERO,
        pressed: 0,
        wayland: false,
        fingers: Vec::new(),
        wheel: Wheel::new(cfg!(target_os = "macos")),
        dropping: None,
        error: None,
        next_update: None,
        prepared: false,
        clipboard: Clipboard::new(),
        preferences: SystemPreferences::default(),
        proxy: event_loop.create_proxy(),
        outcome: outcome.clone(),
    };
    event_loop.run_app(runner)?;
    let finished = outcome.borrow_mut().take();
    Ok(finished.unwrap_or(Ok(()))?)
}

struct Surface {
    window: Arc<dyn Window>,
    touch_cursor: Option<CustomCursor>,
    fullscreen: bool,
    focused: bool,
    accessibility: Accessibility,
}

struct Runner {
    runner: beui_core::runner::Runner,
    size: Vec2,
    #[cfg(all(unix, not(target_os = "macos")))]
    app_id: Option<String>,
    load: Option<Load>,
    surface: Option<Surface>,
    pointer: Pos2,
    pressed: u32,
    wayland: bool,
    fingers: Vec<TouchId>,
    wheel: Wheel,
    dropping: Option<AsyncRequestSerial>,
    error: Option<String>,
    next_update: Option<Instant>,
    prepared: bool,
    clipboard: Clipboard,
    preferences: SystemPreferences,
    proxy: EventLoopProxy,
    outcome: Outcome,
}

impl Drop for Runner {
    fn drop(&mut self) {
        let finished = self.runner.outcome();
        *self.outcome.borrow_mut() = Some(match self.error.take() {
            Some(error) => Err(error),
            None => finished,
        });
    }
}

struct WinitPlatform<'a> {
    surface: &'a mut Surface,
    clipboard: &'a mut Clipboard,
}

impl PlatformServices for WinitPlatform<'_> {
    fn copy(&mut self, text: String) {
        self.clipboard.set(text);
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        self.clipboard.set_image(&png);
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

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        file_saver::open(request, answer);
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        let surface = &*self.surface;
        if window.requests.pointer_locked != shown.requests.pointer_locked {
            lock_pointer(
                &*surface.window,
                window.requests.pointer_locked && surface.focused,
            );
        }
        if (window.requests.cursor, window.requests.pointer_locked)
            != (shown.requests.cursor, shown.requests.pointer_locked)
        {
            show_cursor(surface, window);
        }
        if let Some(request) = ime_request(window, shown) {
            let _ = surface.window.request_ime_update(request);
        }
        if let Some(title) = &window.appearance.title
            && window.appearance.title != shown.appearance.title
        {
            surface.window.set_title(title);
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

fn ime_request(window: &WindowState, shown: &WindowState) -> Option<ImeRequest> {
    fn typed(state: &WindowState) -> Option<&ImeArea> {
        state
            .requests
            .ime
            .as_ref()
            .filter(|area| area.keyboard.shown())
    }
    let (wanted, asked) = (typed(window), typed(shown));
    if wanted == asked {
        return None;
    }
    let Some(area) = wanted else {
        return Some(ImeRequest::Disable);
    };
    let data = ImeRequestData::default().with_cursor_area(
        LogicalPosition::new(area.cursor.min.x, area.cursor.min.y).into(),
        LogicalSize::new(area.cursor.width().max(1.0), area.cursor.height().max(1.0)).into(),
    );
    match asked {
        Some(_) => Some(ImeRequest::Update(data)),
        None => ImeEnableRequest::new(ImeCapabilities::new().with_cursor_area(), data)
            .map(ImeRequest::Enable),
    }
}

impl Runner {
    fn fail(&mut self, event_loop: &dyn ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        self.exit(event_loop);
    }

    fn exit(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.exit();
        event_loop.exit();
    }

    fn push(&mut self, input: impl Into<Incoming>) {
        self.runner.push(input);
    }

    fn locked(&self) -> bool {
        self.runner.window().requests.pointer_locked
            && self.surface.as_ref().is_some_and(|surface| surface.focused)
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn update(&mut self, event_loop: &dyn ActiveEventLoop) -> bool {
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

    fn redraw(&mut self, event_loop: &dyn ActiveEventLoop) {
        if !std::mem::take(&mut self.prepared) || self.runner.has_events() {
            self.update(event_loop);
        }
        if self.runner.present() {
            self.request_redraw();
        }
    }

    fn attach(&mut self) -> Result<(), Box<dyn Error>> {
        let (Some(surface), Some(rendering)) = (&self.surface, self.runner.rendering()) else {
            return Ok(());
        };
        if rendering.attached() {
            return Ok(());
        }
        let size = surface.window.surface_size();
        rendering.attach(Arc::new(surface.window.clone()), size.width, size.height)?;
        surface.window.request_redraw();
        Ok(())
    }

    fn touch(
        &mut self,
        device: Option<DeviceId>,
        finger: FingerId,
        phase: TouchPhase,
        position: Option<PhysicalPosition<f64>>,
        force: Option<Force>,
    ) {
        let id = TouchId {
            device: hash(device),
            finger: finger.into_raw() as u64,
        };
        let held = self.fingers.contains(&id);
        match phase {
            TouchPhase::Start if !held => self.fingers.push(id),
            TouchPhase::Start => return,
            TouchPhase::End | TouchPhase::Cancel if held => {
                self.fingers.retain(|finger| *finger != id);
            }
            TouchPhase::Move if held => {}
            _ => return,
        }
        let pos = position.map_or(self.pointer, physical);
        self.push(Input::Touch {
            id,
            phase,
            pos,
            force: force.map(|force| force.normalized(None) as f32),
        });
    }

    fn pen(
        &mut self,
        phase: PenPhase,
        position: Option<PhysicalPosition<f64>>,
        kind: TabletToolKind,
        data: TabletToolData,
    ) {
        if let Some(position) = position {
            self.pointer = physical(position);
        }
        let pressure = data
            .force
            .map_or(0.0, |force| force.normalized(None) as f32);
        let tilt = data.tilt().map_or(Vec2::ZERO, |tilt| {
            vec2(f32::from(tilt.x), f32::from(tilt.y))
        });
        self.push(Input::Pen {
            phase,
            pos: self.pointer,
            pen: Pen {
                pressure,
                tilt,
                eraser: kind == TabletToolKind::Eraser,
            },
        });
    }

    fn pointer_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::PointerEntered { position, kind, .. } => match kind {
                PointerKind::TabletTool(kind) => {
                    self.pen(
                        PenPhase::Move,
                        Some(position),
                        kind,
                        TabletToolData::default(),
                    );
                }
                PointerKind::Touch(_) => {}
                _ => {
                    self.pointer = physical(position);
                    self.push(Input::PointerMoved(self.pointer));
                }
            },
            WindowEvent::PointerMoved {
                device_id,
                position,
                source,
                ..
            } => match source {
                PointerSource::Touch { finger_id, force } => {
                    self.touch(
                        device_id,
                        finger_id,
                        TouchPhase::Move,
                        Some(position),
                        force,
                    );
                }
                PointerSource::TabletTool { kind, data } => {
                    self.pen(PenPhase::Move, Some(position), kind, data);
                }
                _ => {
                    self.pointer = physical(position);
                    if !self.locked() {
                        self.push(Input::PointerMoved(self.pointer));
                    }
                }
            },
            WindowEvent::PointerLeft {
                device_id,
                position,
                kind,
                ..
            } => match kind {
                PointerKind::Touch(finger) => {
                    self.touch(device_id, finger, TouchPhase::Cancel, position, None);
                }
                PointerKind::TabletTool(kind) => {
                    self.pen(PenPhase::Gone, position, kind, TabletToolData::default());
                }
                _ => {
                    self.push(Input::PointerLeft);
                    if self.wayland && std::mem::take(&mut self.pressed) > 0 {
                        self.push(Input::PointersCancelled);
                    }
                }
            },
            WindowEvent::PointerButton {
                device_id,
                state,
                position,
                button,
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    ButtonSource::Touch { finger_id, force } => {
                        let phase = match pressed {
                            true => TouchPhase::Start,
                            false => TouchPhase::End,
                        };
                        self.touch(device_id, finger_id, phase, Some(position), force);
                    }
                    ButtonSource::TabletTool { kind, button, data } => {
                        let button = pen_button(button);
                        let phase = match pressed {
                            true => PenPhase::Down(button),
                            false => PenPhase::Up(button),
                        };
                        self.pen(phase, Some(position), kind, data);
                    }
                    ButtonSource::Mouse(button) => {
                        self.pointer = physical(position);
                        self.pressed = match pressed {
                            true => self.pressed.saturating_add(1),
                            false => self.pressed.saturating_sub(1),
                        };
                        if let Some(button) = pointer_button(button) {
                            self.push(Input::PointerButton {
                                pos: self.pointer,
                                button,
                                pressed,
                            });
                        }
                    }
                    ButtonSource::Unknown(code) if code >= BUTTON_EVDEV_FIRST => {
                        self.pointer = physical(position);
                        self.pressed = match pressed {
                            true => self.pressed.saturating_add(1),
                            false => self.pressed.saturating_sub(1),
                        };
                        let button = PointerButton::Other(code);
                        self.push(Input::PointerButton {
                            pos: self.pointer,
                            button,
                            pressed,
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for Runner {
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
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

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use winit::platform::wayland::ActiveEventLoopExtWayland;
            self.wayland = event_loop.is_wayland();
        }
        if self.surface.is_some() {
            if let Err(error) = self.attach() {
                self.fail(event_loop, error);
            }
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title(
                self.runner
                    .window()
                    .appearance
                    .title
                    .clone()
                    .unwrap_or_default(),
            )
            .with_visible(false)
            .with_surface_size(LogicalSize::new(self.size.x, self.size.y));
        #[cfg(all(unix, not(target_os = "macos")))]
        let attributes = match &self.app_id {
            Some(app_id) => attributes.with_platform_attributes(Box::new(
                winit::platform::wayland::WindowAttributesWayland::default()
                    .with_name(app_id, app_id),
            )),
            None => attributes,
        };
        let window: Arc<dyn Window> = match event_loop.create_window(attributes) {
            Ok(window) => Arc::from(window),
            Err(error) => return self.fail(event_loop, error),
        };
        let accessibility = Accessibility::new(&*window, self.proxy.clone());
        let touch = beui_core::touch_cursor::image(window.scale_factor() as f32);
        let touch_cursor = CustomCursorSource::from_rgba(
            touch.rgba,
            touch.width as u16,
            touch.height as u16,
            touch.hotspot.0 as u16,
            touch.hotspot.1 as u16,
        )
        .ok()
        .and_then(|source| event_loop.create_custom_cursor(source).ok());
        window.set_visible(true);
        let Some(load) = self.load.take() else {
            return;
        };
        let loaded = match load(Arc::new(window.clone())) {
            Ok(loaded) => loaded,
            Err(error) => return self.fail(event_loop, error),
        };
        self.runner.reports().scaled(window.scale_factor() as f32);
        self.preferences.window_theme(window.theme());
        let proxy = self.proxy.clone();
        self.preferences.watch_desktop(move || proxy.wake_up());
        self.runner
            .reports()
            .system_preferences(self.preferences.layer());
        let proxy = self.proxy.clone();
        let mut setup = Setup::new(Waker::new(move || proxy.wake_up()));
        setup.provide(window.clone());
        self.surface = Some(Surface {
            window,
            touch_cursor,
            fullscreen: false,
            focused: true,
            accessibility,
        });
        if let Err(error) = self.runner.start(loaded, setup) {
            return self.fail(event_loop, error);
        }
        if let Err(error) = self.attach() {
            self.fail(event_loop, error);
        }
    }

    fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
        if let Some(rendering) = self.runner.rendering() {
            rendering.detach();
        }
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        self.runner
            .reports()
            .system_preferences(self.preferences.layer());
        self.prepared = false;
        self.request_redraw();
        let Some(surface) = &self.surface else {
            return;
        };
        for request in surface.accessibility.take_requests() {
            match request {
                Request::InitialTree => self.runner.accessibility_requested(),
                Request::Action(request) => self.runner.accessibility_action(request),
                Request::Deactivated => self.runner.accessibility_deactivated(),
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = self.surface.as_ref().map(|surface| surface.window.id());
        if window != Some(window_id) {
            return;
        }
        if let Some(surface) = &mut self.surface {
            surface.accessibility.window_event(&*surface.window, &event);
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
            WindowEvent::SurfaceResized(size) => {
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
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.pressed = 0;
                    self.push(Input::PointersCancelled);
                }
                if let Some(surface) = &mut self.surface {
                    surface.focused = focused;
                    let state = self.runner.window();
                    if state.requests.pointer_locked {
                        lock_pointer(&*surface.window, focused);
                        show_cursor(surface, state);
                    }
                }
                self.push(Input::Focus(focused));
                if focused {
                    self.runner.lifecycle(Lifecycle::Focused);
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.runner.lifecycle(match occluded {
                    true => Lifecycle::Background,
                    false => Lifecycle::Foreground,
                });
            }
            WindowEvent::ThemeChanged(theme) => {
                self.preferences.window_theme(Some(theme));
                self.runner
                    .reports()
                    .system_preferences(self.preferences.layer());
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.push(Input::Modifiers(Modifiers::with_meta(
                    state.alt_key(),
                    state.control_key(),
                    state.shift_key(),
                    state.meta_key(),
                    cfg!(target_os = "macos"),
                )));
            }
            WindowEvent::PointerEntered { .. }
            | WindowEvent::PointerMoved { .. }
            | WindowEvent::PointerLeft { .. }
            | WindowEvent::PointerButton { .. } => {
                self.pointer_event(event);
                self.push(Incoming::Time(Instant::now()));
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let wheeled = self.wheel.event(phase, Instant::now());
                if wheeled == Wheeled::Ignore {
                    return;
                }
                if wheeled == Wheeled::StartAndScroll {
                    self.push(Input::ScrollStarted);
                }
                self.push(Input::Scroll(match delta {
                    MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines(vec2(x, y)),
                    MouseScrollDelta::PixelDelta(position) => {
                        ScrollDelta::Pixels(physical(position).to_vec2())
                    }
                    _ => return,
                }));
                if wheeled == Wheeled::ScrollAndEnd {
                    self.push(Input::ScrollEnded);
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.push(Input::Zoom(1.0 + delta as f32));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let evdev = evdev(event.physical_key);
                let key = logical_key(
                    &event.key_without_modifiers,
                    evdev.and_then(Key::from_evdev),
                );
                self.push(Input::Key(Keystroke {
                    key,
                    pressed: event.state == ElementState::Pressed,
                    repeat: event.repeat,
                    evdev,
                    text: event.text.map(|text| text.to_string()),
                }));
            }
            WindowEvent::Ime(Ime::Enabled) => self.push(Input::Ime(ImeEvent::Enabled)),
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.push(Input::Ime(ImeEvent::SetComposingText(text)));
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.push(Input::Ime(ImeEvent::CommitText(text)))
            }
            WindowEvent::Ime(Ime::Disabled) => {
                self.push(Input::Ime(ImeEvent::SetComposingText(String::new())));
                self.push(Input::Ime(ImeEvent::Disabled));
            }
            WindowEvent::DragEntered { id, .. } => {
                let files = event_loop
                    .data_transfer(id)
                    .is_ok_and(|transfer| transfer.has_type(&TypeHint::UriList));
                if files
                    && event_loop
                        .set_valid_dnd_actions(id, &[DndAction::Copy])
                        .is_ok()
                {
                    self.push(FileDrag::Hovered);
                }
            }
            WindowEvent::DragLeft { .. } => {
                self.dropping = None;
                self.push(FileDrag::Cancelled);
            }
            WindowEvent::DragDropped { id, .. } => {
                self.dropping = event_loop.fetch_data_transfer(id, &TypeHint::UriList).ok();
                if self.dropping.is_none() {
                    self.push(FileDrag::Cancelled);
                }
            }
            WindowEvent::DataTransferReceived { serial, value, .. }
                if self.dropping == Some(serial) =>
            {
                self.dropping = None;
                let paths = value.try_as_file_paths().unwrap_or_default();
                if paths.is_empty() {
                    self.push(FileDrag::Cancelled);
                }
                for path in paths {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_owned();
                    self.push(FileDrag::Dropped(DroppedFile {
                        name,
                        path: Some(path),
                        bytes: None,
                    }));
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        _device: Option<DeviceId>,
        event: DeviceEvent,
    ) {
        let DeviceEvent::PointerMotion { delta } = event else {
            return;
        };
        if !self.locked() {
            return;
        }
        self.push(Input::PointerMotion(vec2(delta.0 as f32, delta.1 as f32)));
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

fn pen_button(button: TabletToolButton) -> PointerButton {
    match button {
        TabletToolButton::Contact => PointerButton::Primary,
        TabletToolButton::Barrel => PointerButton::Secondary,
        TabletToolButton::Other(code) => Option::<MouseButton>::from(button)
            .and_then(pointer_button)
            .unwrap_or(PointerButton::Other(code)),
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back => Some(PointerButton::Back),
        MouseButton::Forward => Some(PointerButton::Forward),
        button => Some(PointerButton::Other(BUTTON_EVDEV_FIRST + button as u16)),
    }
}

fn cursor(icon: CursorIcon) -> Option<winit::cursor::CursorIcon> {
    Some(match icon {
        CursorIcon::Default => winit::cursor::CursorIcon::Default,
        CursorIcon::Crosshair | CursorIcon::Touch => winit::cursor::CursorIcon::Crosshair,
        CursorIcon::Grab => winit::cursor::CursorIcon::Grab,
        CursorIcon::Grabbing => winit::cursor::CursorIcon::Grabbing,
        CursorIcon::NotAllowed => winit::cursor::CursorIcon::NotAllowed,
        CursorIcon::PointingHand => winit::cursor::CursorIcon::Pointer,
        CursorIcon::ResizeHorizontal => winit::cursor::CursorIcon::EwResize,
        CursorIcon::ResizeVertical => winit::cursor::CursorIcon::NsResize,
        CursorIcon::ResizeNeSw => winit::cursor::CursorIcon::NeswResize,
        CursorIcon::ResizeNwSe => winit::cursor::CursorIcon::NwseResize,
        CursorIcon::Text => winit::cursor::CursorIcon::Text,
        CursorIcon::Wait => winit::cursor::CursorIcon::Wait,
        CursorIcon::Move => winit::cursor::CursorIcon::Move,
        CursorIcon::Progress => winit::cursor::CursorIcon::Progress,
        CursorIcon::Help => winit::cursor::CursorIcon::Help,
        CursorIcon::Alias => winit::cursor::CursorIcon::Alias,
        CursorIcon::ContextMenu => winit::cursor::CursorIcon::ContextMenu,
        CursorIcon::Cell => winit::cursor::CursorIcon::Cell,
        CursorIcon::VerticalText => winit::cursor::CursorIcon::VerticalText,
        CursorIcon::Copy => winit::cursor::CursorIcon::Copy,
        CursorIcon::NoDrop => winit::cursor::CursorIcon::NoDrop,
        CursorIcon::AllScroll => winit::cursor::CursorIcon::AllScroll,
        CursorIcon::ZoomIn => winit::cursor::CursorIcon::ZoomIn,
        CursorIcon::ZoomOut => winit::cursor::CursorIcon::ZoomOut,
        CursorIcon::ResizeColumn => winit::cursor::CursorIcon::ColResize,
        CursorIcon::ResizeRow => winit::cursor::CursorIcon::RowResize,
        CursorIcon::ResizeNorth => winit::cursor::CursorIcon::NResize,
        CursorIcon::ResizeEast => winit::cursor::CursorIcon::EResize,
        CursorIcon::ResizeSouth => winit::cursor::CursorIcon::SResize,
        CursorIcon::ResizeWest => winit::cursor::CursorIcon::WResize,
        CursorIcon::ResizeNorthEast => winit::cursor::CursorIcon::NeResize,
        CursorIcon::ResizeNorthWest => winit::cursor::CursorIcon::NwResize,
        CursorIcon::ResizeSouthEast => winit::cursor::CursorIcon::SeResize,
        CursorIcon::ResizeSouthWest => winit::cursor::CursorIcon::SwResize,
        CursorIcon::None => return None,
    })
}

fn evdev(physical: PhysicalKey) -> Option<u32> {
    match physical {
        PhysicalKey::Code(code) => Key::evdev_from_code(&code.to_string()),
        PhysicalKey::Unidentified(NativeKeyCode::Xkb(code)) => code.checked_sub(8),
        PhysicalKey::Unidentified(NativeKeyCode::Android(code)) => Some(code),
        PhysicalKey::Unidentified(_) => None,
    }
}

fn logical_key(logical: &winit::keyboard::Key, physical: Option<Key>) -> Option<Key> {
    use winit::keyboard::Key as Winit;
    let apple = cfg!(target_os = "macos");
    match logical {
        Winit::Named(named) => Key::from_value(&named.to_string(), physical, apple),
        Winit::Character(text) => Key::from_value(text, physical, apple),
        Winit::Unidentified(winit::keyboard::NativeKey::Xkb(XKB_AUDIO_MIC_MUTE)) => {
            Some(Key::MicMute)
        }
        Winit::Unidentified(_) | Winit::Dead(_) => physical,
    }
}

fn show_cursor(surface: &Surface, state: &WindowState) {
    let window = &surface.window;
    let locked = state.requests.pointer_locked && surface.focused;
    if state.requests.cursor == CursorIcon::Touch
        && let Some(touch) = &surface.touch_cursor
    {
        window.set_cursor(touch.clone().into());
        window.set_cursor_visible(!locked);
        return;
    }
    match cursor(state.requests.cursor) {
        Some(icon) => {
            window.set_cursor(icon.into());
            window.set_cursor_visible(!locked);
        }
        None => window.set_cursor_visible(false),
    }
}

fn lock_pointer(window: &dyn Window, locked: bool) {
    let grab = match locked {
        true => CursorGrabMode::Locked,
        false => CursorGrabMode::None,
    };
    if window.set_cursor_grab(grab).is_err() && locked {
        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
    }
    window.set_cursor_visible(!locked);
}
