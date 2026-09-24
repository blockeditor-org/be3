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
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{
    CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, Window, WindowId,
};

use super::clipboard::Clipboard;
use super::{App, RunOptions, Setup, Waker};
use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::input::{
    CursorIcon, DroppedFile, Event, ImeArea, ImeEvent, Key, Modifiers, PointerButton, RawInput,
    TouchId, TouchPhase,
};
use crate::renderer::{Renderer, RendererInfo, Repaint, clear_color};

const LINE_HEIGHT: f32 = 40.0;
const TOUCH_CURSOR_SIZE: u16 = 20;
const TOUCH_CURSOR_RADIUS: f32 = TOUCH_CURSOR_SIZE as f32 / 2.0;
const TOUCH_CURSOR_STROKE: f32 = 1.0;
const TOUCH_CURSOR_SAMPLES: u16 = 4;

enum UserEvent {
    AccessKit(AccessKitEvent),
    Wake,
}

impl From<AccessKitEvent> for UserEvent {
    fn from(event: AccessKitEvent) -> Self {
        Self::AccessKit(event)
    }
}

pub fn run(title: impl Into<String>, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::new(title), app)
}

pub fn run_with(options: RunOptions, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "android")]
    let mut options = options;
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "android")]
    if let Some(android_app) = options.android_app.take() {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        builder.with_android_app(android_app);
    }
    let event_loop = builder.build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = Runner {
        options,
        app: Box::new(app),
        context: Context::new(),
        gpu: None,
        surface: None,
        events: Vec::new(),
        modifiers: Modifiers::NONE,
        pointer: Pos2::ZERO,
        emulated_touch: false,
        held_buttons: 0,
        pointer_left: false,
        error: None,
        next_update: None,
        clipboard: Clipboard::new(),
        event_loop_proxy: event_loop.create_proxy(),
        accessibility_active: false,
        exiting: false,
    };
    event_loop.run_app(&mut runner)?;
    match runner.error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    renderer: Renderer,
}

struct Surface {
    window: Arc<Window>,
    target: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    cursor_icon: CursorIcon,
    pointer_locked: bool,
    touch_emulation: bool,
    touch_cursor: CustomCursor,
    ime: Option<ImeArea>,
    fullscreen: bool,
    prepared_size: Option<(Vec2, f32)>,
    clear_color: Option<Color32>,
    pending: Option<Repaint>,
    retained: Option<Retained>,
    accessibility: AccessKitAdapter,
}

struct Retained {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Surface {
    fn retain(&mut self, device: &wgpu::Device) {
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_DST) {
            return;
        }
        let size = (self.config.width, self.config.height);
        if self
            .retained
            .as_ref()
            .is_none_or(|retained| retained.size != size)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("beui retained frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.retained = Some(Retained {
                texture,
                view,
                size,
            });
        }
    }

    fn retains(&self) -> bool {
        self.retained
            .as_ref()
            .is_some_and(|retained| retained.size == (self.config.width, self.config.height))
    }

    fn configure(&mut self, device: &wgpu::Device) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        if let Some(target) = &self.target {
            target.configure(device, &self.config);
        }
    }
}

struct Runner {
    options: RunOptions,
    app: Box<dyn App>,
    context: Context,
    gpu: Option<Gpu>,
    surface: Option<Surface>,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer: Pos2,
    emulated_touch: bool,
    held_buttons: u8,
    pointer_left: bool,
    error: Option<String>,
    next_update: Option<Instant>,
    clipboard: Clipboard,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    accessibility_active: bool,
    exiting: bool,
}

impl Runner {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        self.exit(event_loop);
    }

    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        if !self.exiting {
            self.exiting = true;
            self.app.exiting();
        }
        event_loop.exit();
    }

    fn push(&mut self, event: Event) {
        self.events.push(event);
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
        self.context
            .simulated_pixels_per_point()
            .map_or(native * f64::from(self.context.zoom_factor()), f64::from)
    }

    fn logical(&self, position: PhysicalPosition<f64>) -> Pos2 {
        let scale = self.scale_factor();
        pos2((position.x / scale) as f32, (position.y / scale) as f32)
    }

    fn update(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let (Some(surface), Some(gpu)) = (&mut self.surface, &mut self.gpu) else {
            return false;
        };
        if surface.target.is_none() || surface.config.width == 0 || surface.config.height == 0 {
            self.events.clear();
            self.next_update = None;
            return false;
        }

        self.context
            .set_pixels_per_point(surface.window.scale_factor() as f32);
        self.context
            .set_accessibility_active(self.accessibility_active);
        self.context.set_test_ids_published(false);
        let scale = self.context.pixels_per_point();
        let physical = vec2(surface.config.width as f32, surface.config.height as f32);
        let screen = vec2(physical.x / scale, physical.y / scale);

        let raw = RawInput {
            events: super::next_batch(&mut self.events),
        };
        let app = &mut self.app;
        let output = self.context.run(raw, |context| {
            app.update(context, Rect::from_min_size(Pos2::ZERO, screen));
        });
        if self.accessibility_active {
            surface
                .accessibility
                .update_if_active(|| output.accessibility_tree(&self.options.title, screen));
        }

        if let Some(text) = &output.copied_text {
            self.clipboard.set(text.clone());
        }
        if output.paste_requested
            && let Some(text) = self.clipboard.get()
        {
            self.events.push(Event::Text(text));
            surface.window.request_redraw();
        }
        if output.pointer_locked != surface.pointer_locked {
            surface.pointer_locked = output.pointer_locked;
            lock_pointer(&surface.window, output.pointer_locked);
        }
        let touch_emulation = self.context.touch_emulation();
        if output.cursor_icon != surface.cursor_icon || touch_emulation != surface.touch_emulation {
            surface.cursor_icon = output.cursor_icon;
            surface.touch_emulation = touch_emulation;
            if touch_emulation {
                surface.window.set_cursor(surface.touch_cursor.clone());
                surface.window.set_cursor_visible(true);
            } else {
                match cursor(output.cursor_icon) {
                    Some(icon) => {
                        surface.window.set_cursor(icon);
                        surface.window.set_cursor_visible(!surface.pointer_locked);
                    }
                    None => surface.window.set_cursor_visible(false),
                }
            }
        }
        if output.ime != surface.ime {
            if output.ime.is_some() != surface.ime.is_some() {
                surface.window.set_ime_allowed(output.ime.is_some());
            }
            if let Some(area) = output.ime {
                surface.window.set_ime_cursor_area(
                    LogicalPosition::new(area.cursor.min.x, area.cursor.min.y),
                    LogicalSize::new(area.cursor.width().max(1.0), area.cursor.height().max(1.0)),
                );
            }
            surface.ime = output.ime;
        }
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != surface.fullscreen
        {
            surface.fullscreen = fullscreen;
            surface
                .window
                .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }

        let size = (physical, scale);
        let clear_color = self.app.clear_color();
        let stale = surface.prepared_size != Some(size)
            || surface.clear_color != Some(clear_color)
            || !surface.retains();
        let repaint = match output.damage() {
            Some(region) if !stale => Repaint::Region {
                region,
                background: clear_color,
            },
            _ => Repaint::Everything,
        };
        if output.changed || stale {
            let repaint = match surface.pending {
                Some(pending) => pending.union(repaint),
                None => repaint,
            };
            let effective =
                gpu.renderer
                    .prepare(&gpu.device, &gpu.queue, &output, physical, scale, repaint);
            surface.prepared_size = Some(size);
            surface.pending = Some(effective);
        }
        surface.clear_color = Some(clear_color);
        self.next_update = Instant::now().checked_add(output.repaint_after);
        let pending = surface.pending.is_some();
        if output.close_requested {
            self.exit(event_loop);
        }
        pending
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        self.update(event_loop);
        let (Some(surface), Some(gpu)) = (&mut self.surface, &mut self.gpu) else {
            return;
        };
        if surface.config.width == 0 || surface.config.height == 0 {
            return;
        }
        let Some(target) = &surface.target else {
            return;
        };

        let frame = match target.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                target.configure(&gpu.device, &surface.config);
                surface.window.request_redraw();
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = clear_color(self.app.clear_color());
        surface.retain(&gpu.device);
        let pending = surface.pending.take();
        let size = (surface.config.width, surface.config.height);
        let retained = surface.retained.as_ref();
        let (target, load) = match retained {
            Some(retained) => (
                &retained.view,
                match pending {
                    Some(Repaint::Region { .. }) => wgpu::LoadOp::Load,
                    _ => wgpu::LoadOp::Clear(clear),
                },
            ),
            None => (&view, wgpu::LoadOp::Clear(clear)),
        };
        if pending.is_some() || retained.is_none() {
            gpu.renderer
                .render(&gpu.device, &gpu.queue, &mut encoder, target, size, load);
        }
        if let Some(retained) = retained {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &retained.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: retained.size.0,
                    height: retained.size.1,
                    depth_or_array_layers: 1,
                },
            );
        }
        gpu.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn attach(&mut self) -> Result<(), Box<dyn Error>> {
        let (Some(surface), Some(gpu)) = (&mut self.surface, &self.gpu) else {
            return Ok(());
        };
        if surface.target.is_some() {
            return Ok(());
        }
        let size = surface.window.inner_size();
        let target = gpu.instance.create_surface(surface.window.clone())?;
        let mut config = target
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or("the adapter does not support this surface")?;
        config.format = gpu.format;
        let capabilities = target.get_capabilities(&gpu.adapter);
        if capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        config.width = size.width;
        config.height = size.height;
        surface.config = config;
        surface.target = Some(target);
        surface.retained = None;
        surface.prepared_size = None;
        surface.configure(&gpu.device);
        surface.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler<UserEvent> for Runner {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if (!self.events.is_empty()
            || self
                .next_update
                .is_some_and(|deadline| deadline <= Instant::now()))
            && self.update(event_loop)
        {
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
            .with_title(self.options.title.clone())
            .with_visible(false)
            .with_inner_size(LogicalSize::new(self.options.size.x, self.options.size.y));
        #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
        let attributes = match &self.options.app_id {
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
        let gpu = match pollster::block_on(create_gpu(window.clone(), &self.context)) {
            Ok(gpu) => gpu,
            Err(error) => return self.fail(event_loop, error),
        };
        let proxy = self.event_loop_proxy.clone();
        let setup = Setup {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            format: gpu.format,
            waker: Waker::new(move || {
                let _ = proxy.send_event(UserEvent::Wake);
            }),
            window: window.clone(),
        };
        self.surface = Some(Surface {
            window,
            target: None,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: gpu.format,
                width: 0,
                height: 0,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: Vec::new(),
            },
            cursor_icon: CursorIcon::Default,
            pointer_locked: false,
            touch_emulation: false,
            touch_cursor,
            ime: None,
            fullscreen: false,
            prepared_size: None,
            clear_color: None,
            pending: None,
            retained: None,
            accessibility,
        });
        self.gpu = Some(gpu);
        if let Err(error) = self.attach() {
            return self.fail(event_loop, error);
        }
        self.app.setup(&setup);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(surface) = &mut self.surface {
            surface.target = None;
            surface.retained = None;
            surface.pending = None;
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
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
                self.context.reset_accessibility();
                self.request_redraw();
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                self.context.accessibility_action(request);
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
        match event {
            WindowEvent::CloseRequested => {
                if self.app.close_requested() {
                    self.exit(event_loop);
                } else {
                    self.request_redraw();
                }
            }
            WindowEvent::Destroyed => self.exit(event_loop),
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(gpu)) = (&mut self.surface, &self.gpu) {
                    surface.config.width = size.width;
                    surface.config.height = size.height;
                    surface.configure(&gpu.device);
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
                self.modifiers = Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key() || state.super_key(),
                    shift: state.shift_key(),
                };
                self.push(Event::Modifiers(self.modifiers));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = self.logical(position);
                if self.context.touch_emulation() {
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
                if self.context.touch_emulation() {
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
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(x * LINE_HEIGHT, y * LINE_HEIGHT),
                    MouseScrollDelta::PixelDelta(position) => {
                        vec2(position.x as f32, position.y as f32)
                    }
                };
                self.push(Event::Scroll(delta));
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
                if let PhysicalKey::Code(code) = event.physical_key {
                    if pressed
                        && code == KeyCode::KeyV
                        && self.modifiers.ctrl
                        && !self.modifiers.alt
                        && let Some(text) = self.clipboard.get()
                    {
                        self.push(Event::Text(text));
                    }
                    if let Some(key) = key(code) {
                        self.push(Event::Key {
                            key,
                            pressed,
                            repeat: event.repeat,
                            modifiers: self.modifiers,
                        });
                    }
                }
                if pressed
                    && !self.modifiers.ctrl
                    && !self.modifiers.alt
                    && let Some(text) = event.text
                    && !text.chars().any(char::is_control)
                {
                    self.push(Event::Text(text.to_string()));
                }
            }
            WindowEvent::Ime(ime) => self.push(Event::Ime(match ime {
                Ime::Enabled => ImeEvent::Enabled,
                Ime::Preedit(text, _) => ImeEvent::Preedit(text),
                Ime::Commit(text) => ImeEvent::Commit(text),
                Ime::Disabled => ImeEvent::Disabled,
            })),
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
        if !self.context.pointer_locked() {
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

async fn create_gpu(window: Arc<Window>, context: &Context) -> Result<Gpu, Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let probe = instance.create_surface(window)?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&probe),
        })
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("beui device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await?;
    let capabilities = probe.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or("the adapter does not support this surface")?;
    drop(probe);
    let renderer = Renderer::new(&device, format);
    context.set_renderer_info(RendererInfo {
        adapter: adapter.get_info(),
        format,
    });
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
        format,
        renderer,
    })
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
