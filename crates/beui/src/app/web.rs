use std::cell::{Cell, RefCell};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

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
const PAGE_HEIGHT: f32 = 800.0;
const PINCH_SPEED: f32 = 0.01;

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

struct Input {
    events: RefCell<Vec<Event>>,
    modifiers: Cell<Modifiers>,
    scale: Cell<f32>,
    locked: Cell<bool>,
    buttons: Cell<u16>,
    scheduled: Cell<bool>,
}

thread_local! {
    static INPUT: Input = const { Input {
        events: RefCell::new(Vec::new()),
        modifiers: Cell::new(Modifiers::NONE),
        scale: Cell::new(1.0),
        locked: Cell::new(false),
        buttons: Cell::new(0),
        scheduled: Cell::new(false),
    } };
    static RUNNER: RefCell<Option<Rc<RefCell<Runner>>>> = const { RefCell::new(None) };
    static FRAME: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
    static TIMEOUT: Cell<Option<i32>> = const { Cell::new(None) };
}

fn push(event: Event) {
    INPUT.with(|input| input.events.borrow_mut().push(event));
    schedule();
}

fn schedule() {
    let first = INPUT.with(|input| !input.scheduled.replace(true));
    if !first {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    FRAME.with(|frame| {
        if let Some(frame) = frame.borrow().as_ref() {
            let _ = window.request_animation_frame(frame.as_ref().unchecked_ref());
        }
    });
}

fn schedule_after(delay: Duration) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Some(previous) = TIMEOUT.take() {
        window.clear_timeout_with_handle(previous);
    }
    let milliseconds = delay.as_millis().min(i32::MAX as u128) as i32;
    TIMER.with(|timer| {
        if let Some(timer) = timer.borrow().as_ref() {
            TIMEOUT.set(
                window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        timer.as_ref().unchecked_ref(),
                        milliseconds,
                    )
                    .ok(),
            );
        }
    });
}

fn run_frame() {
    INPUT.with(|input| input.scheduled.set(false));
    let runner = RUNNER.with(|runner| runner.borrow().clone());
    if let Some(runner) = runner {
        runner.borrow_mut().frame();
    }
}

struct Runner {
    app: Box<dyn App>,
    context: Context,
    canvas: web_sys::HtmlCanvasElement,
    agent: web_sys::HtmlTextAreaElement,
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    cursor_icon: CursorIcon,
    pointer_locked: bool,
    ime: Option<ImeArea>,
    fullscreen: bool,
    prepared: Option<(Vec2, f32, Color32)>,
}

impl Runner {
    fn frame(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Some(previous) = TIMEOUT.take() {
            window.clear_timeout_with_handle(previous);
        }
        let ratio = window.device_pixel_ratio() as f32;
        let bounds = self.canvas.get_bounding_client_rect();
        let width = ((bounds.width() as f32) * ratio).round().max(1.0) as u32;
        let height = ((bounds.height() as f32) * ratio).round().max(1.0) as u32;
        if self.config.width != width || self.config.height != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
        self.context.set_pixels_per_point(ratio);
        let scale = self
            .context
            .simulated_pixels_per_point()
            .unwrap_or(self.context.pixels_per_point());
        INPUT.with(|input| input.scale.set(ratio / scale));
        let physical = vec2(width as f32, height as f32);
        let screen = physical / scale;
        let (events, deferred) = INPUT.with(|input| {
            let mut pending = input.events.borrow_mut();
            let events = super::next_batch(&mut pending);
            (events, !pending.is_empty())
        });
        let app = &mut self.app;
        let output = self.context.run(RawInput { events }, |context| {
            app.update(context, Rect::from_min_size(Pos2::ZERO, screen));
        });

        if let Some(text) = &output.copied_text {
            let _ = window.navigator().clipboard().write_text(text);
        }
        if output.paste_requested {
            let read = window.navigator().clipboard().read_text();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(text) = wasm_bindgen_futures::JsFuture::from(read).await
                    && let Some(text) = text.as_string()
                {
                    push(Event::Text(text));
                }
            });
        }
        if output.pointer_locked != self.pointer_locked {
            self.pointer_locked = output.pointer_locked;
            match output.pointer_locked {
                true => self.canvas.request_pointer_lock(),
                false => {
                    if let Some(document) = window.document() {
                        document.exit_pointer_lock();
                    }
                }
            }
        }
        if output.cursor_icon != self.cursor_icon {
            self.cursor_icon = output.cursor_icon;
            let _ = self
                .canvas
                .style()
                .set_property("cursor", cursor(output.cursor_icon));
        }
        if output.ime != self.ime {
            self.ime = output.ime;
            if let Some(area) = output.ime {
                let css = 1.0 / INPUT.with(|input| input.scale.get());
                let style = self.agent.style();
                let _ = style.set_property("left", &format!("{}px", area.cursor.min.x * css));
                let _ = style.set_property("top", &format!("{}px", area.cursor.max.y * css));
            }
        }
        if let Some(fullscreen) = output.fullscreen
            && fullscreen != self.fullscreen
        {
            self.fullscreen = fullscreen;
            if let Some(document) = window.document() {
                match fullscreen {
                    true => {
                        if let Some(root) = document.document_element() {
                            let _ = root.request_fullscreen();
                        }
                    }
                    false => document.exit_fullscreen(),
                }
            }
        }

        let background = self.app.clear_color();
        let stale = self.prepared != Some((physical, scale, background));
        if output.changed || stale {
            self.renderer.prepare(
                &self.device,
                &self.queue,
                &output,
                physical,
                scale,
                Repaint::Everything,
            );
            self.prepared = Some((physical, scale, background));
            self.present(background);
        }

        if deferred || output.repaint || output.repaint_after.is_zero() {
            schedule();
        } else if output.repaint_after != Duration::MAX {
            schedule_after(output.repaint_after);
        }
    }

    fn present(&mut self, background: Color32) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.prepared = None;
                schedule();
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = match self.config.format.is_srgb() {
            true => clear_color(background),
            false => {
                let [red, green, blue, alpha] = background.to_array();
                wgpu::Color {
                    r: f64::from(red) / 255.0,
                    g: f64::from(green) / 255.0,
                    b: f64::from(blue) / 255.0,
                    a: f64::from(alpha) / 255.0,
                }
            }
        };
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            wgpu::LoadOp::Clear(clear),
        );
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

struct Wake {
    woken: AtomicBool,
    waker: Mutex<Option<std::task::Waker>>,
}

struct Woken(Arc<Wake>);

impl Future for Woken {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<()> {
        if self.0.woken.swap(false, Ordering::AcqRel) {
            return Poll::Ready(());
        }
        if let Ok(mut waker) = self.0.waker.lock() {
            *waker = Some(context.waker().clone());
        }
        match self.0.woken.swap(false, Ordering::AcqRel) {
            true => Poll::Ready(()),
            false => Poll::Pending,
        }
    }
}

fn waker() -> Waker {
    let wake = Arc::new(Wake {
        woken: AtomicBool::new(false),
        waker: Mutex::new(None),
    });
    let listened = wake.clone();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            Woken(listened.clone()).await;
            schedule();
        }
    });
    Waker::new(move || {
        wake.woken.store(true, Ordering::Release);
        let waiting = wake.waker.lock().ok().and_then(|mut waker| waker.take());
        if let Some(waiting) = waiting {
            waiting.wake();
        }
    })
}

pub async fn run_web(
    canvas_id: &str,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let document = window
        .document()
        .ok_or("no browser document is available")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| format!("no element has the id {canvas_id}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("the element {canvas_id} is not a canvas"))?;
    document.set_title(&options.title);
    let agent = text_agent(&document)?;

    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
    descriptor.display = Some(Box::new(Display));
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|error| error.to_string())?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .map_err(|error| error.to_string())?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("beui device"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|error| error.to_string())?;
    let capabilities = surface.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or("the adapter cannot show a canvas")?;
    let alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let context = Context::new();
    context.set_renderer_info(RendererInfo {
        adapter: adapter.get_info(),
        format,
    });
    let renderer = Renderer::new(&device, format);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: 0,
        height: 0,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: Vec::new(),
    };

    let mut app: Box<dyn App> = Box::new(app);
    app.setup(&Setup {
        device: device.clone(),
        queue: queue.clone(),
        format,
        waker: waker(),
    });
    let runner = Runner {
        app,
        context,
        canvas: canvas.clone(),
        agent: agent.clone(),
        _instance: instance,
        _adapter: adapter,
        device,
        queue,
        surface,
        config,
        renderer,
        cursor_icon: CursorIcon::Default,
        pointer_locked: false,
        ime: None,
        fullscreen: false,
        prepared: None,
    };
    RUNNER.with(|slot| *slot.borrow_mut() = Some(Rc::new(RefCell::new(runner))));
    FRAME.with(|frame| *frame.borrow_mut() = Some(Closure::new(run_frame)));
    TIMER.with(|timer| {
        *timer.borrow_mut() = Some(Closure::new(|| {
            TIMEOUT.set(None);
            schedule();
        }))
    });
    listen(&window, &document, &canvas, &agent)?;
    schedule();
    Ok(())
}

fn text_agent(
    document: &web_sys::Document,
) -> Result<web_sys::HtmlTextAreaElement, Box<dyn Error>> {
    let agent = document
        .create_element("textarea")
        .map_err(|_| "could not create the text input")?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .map_err(|_| "could not create the text input")?;
    agent.set_attribute("autocapitalize", "off").ok();
    agent.set_attribute("autocomplete", "off").ok();
    agent.set_attribute("autocorrect", "off").ok();
    agent.set_attribute("spellcheck", "false").ok();
    agent.set_attribute("aria-hidden", "true").ok();
    let style = agent.style();
    for (property, value) in [
        ("position", "absolute"),
        ("left", "0px"),
        ("top", "0px"),
        ("width", "1px"),
        ("height", "1px"),
        ("opacity", "0"),
        ("border", "none"),
        ("padding", "0"),
        ("resize", "none"),
        ("outline", "none"),
        ("background", "transparent"),
        ("color", "transparent"),
        ("caret-color", "transparent"),
        ("pointer-events", "none"),
        ("z-index", "-1"),
    ] {
        let _ = style.set_property(property, value);
    }
    document
        .body()
        .ok_or("the page has no body")?
        .append_child(&agent)
        .map_err(|_| "could not add the text input to the page")?;
    Ok(agent)
}

fn on<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) -> Result<(), Box<dyn Error>> {
    let closure = Closure::<dyn FnMut(E)>::new(handler);
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            name,
            closure.as_ref().unchecked_ref(),
            &options,
        )
        .map_err(|_| format!("could not listen for {name}"))?;
    closure.forget();
    Ok(())
}

fn position(canvas: &web_sys::HtmlCanvasElement, x: i32, y: i32) -> Pos2 {
    let bounds = canvas.get_bounding_client_rect();
    let scale = INPUT.with(|input| input.scale.get());
    pos2(
        (x as f32 - bounds.left() as f32) * scale,
        (y as f32 - bounds.top() as f32) * scale,
    )
}

fn modifiers_of(alt: bool, ctrl: bool, meta: bool, shift: bool) -> Modifiers {
    let modifiers = Modifiers {
        alt,
        ctrl: ctrl || meta,
        shift,
    };
    let changed = INPUT.with(|input| input.modifiers.replace(modifiers) != modifiers);
    if changed {
        push(Event::Modifiers(modifiers));
    }
    modifiers
}

fn listen(
    window: &web_sys::Window,
    document: &web_sys::Document,
    canvas: &web_sys::HtmlCanvasElement,
    agent: &web_sys::HtmlTextAreaElement,
) -> Result<(), Box<dyn Error>> {
    let canvas_target: &web_sys::EventTarget = canvas.as_ref();
    let agent_target: &web_sys::EventTarget = agent.as_ref();

    on(canvas_target, "pointerdown", {
        let canvas = canvas.clone();
        let agent = agent.clone();
        move |event: web_sys::PointerEvent| {
            event.prevent_default();
            let _ = agent.focus();
            let _ = canvas.set_pointer_capture(event.pointer_id());
            let modifiers = modifiers_of(
                event.alt_key(),
                event.ctrl_key(),
                event.meta_key(),
                event.shift_key(),
            );
            let pos = position(&canvas, event.client_x(), event.client_y());
            if event.pointer_type() == "touch" {
                push(touch(&event, TouchPhase::Start, pos));
                return;
            }
            let Some(button) = pointer_button(event.button()) else {
                return;
            };
            INPUT.with(|input| input.buttons.set(event.buttons()));
            push(Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers,
            });
        }
    })?;
    on(canvas_target, "pointermove", {
        let canvas = canvas.clone();
        move |event: web_sys::PointerEvent| {
            if INPUT.with(|input| input.locked.get()) {
                let scale = INPUT.with(|input| input.scale.get());
                push(Event::PointerMotion(vec2(
                    event.movement_x() as f32 * scale,
                    event.movement_y() as f32 * scale,
                )));
                return;
            }
            let pos = position(&canvas, event.client_x(), event.client_y());
            if event.pointer_type() == "touch" {
                push(touch(&event, TouchPhase::Move, pos));
                return;
            }
            push(Event::PointerMoved(pos));
        }
    })?;
    for (name, cancelled) in [("pointerup", false), ("pointercancel", true)] {
        on(canvas_target, name, {
            let canvas = canvas.clone();
            move |event: web_sys::PointerEvent| {
                let _ = canvas.release_pointer_capture(event.pointer_id());
                let modifiers = modifiers_of(
                    event.alt_key(),
                    event.ctrl_key(),
                    event.meta_key(),
                    event.shift_key(),
                );
                let pos = position(&canvas, event.client_x(), event.client_y());
                if event.pointer_type() == "touch" {
                    let phase = match cancelled {
                        true => TouchPhase::Cancel,
                        false => TouchPhase::End,
                    };
                    push(touch(&event, phase, pos));
                    return;
                }
                INPUT.with(|input| input.buttons.set(event.buttons()));
                let Some(button) = pointer_button(event.button()) else {
                    return;
                };
                push(Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    modifiers,
                });
            }
        })?;
    }
    on(
        canvas_target,
        "pointerleave",
        |event: web_sys::PointerEvent| {
            if event.pointer_type() != "touch" && INPUT.with(|input| input.buttons.get()) == 0 {
                push(Event::PointerGone);
            }
        },
    )?;
    on(canvas_target, "wheel", |event: web_sys::WheelEvent| {
        event.prevent_default();
        let unit = match event.delta_mode() {
            web_sys::WheelEvent::DOM_DELTA_LINE => LINE_HEIGHT,
            web_sys::WheelEvent::DOM_DELTA_PAGE => PAGE_HEIGHT,
            _ => INPUT.with(|input| input.scale.get()),
        };
        let delta = vec2(event.delta_x() as f32, event.delta_y() as f32) * unit;
        if event.ctrl_key() {
            push(Event::Zoom((-delta.y * PINCH_SPEED).exp()));
            return;
        }
        push(Event::Scroll(-delta));
    })?;
    on(
        canvas_target,
        "contextmenu",
        |event: web_sys::MouseEvent| {
            event.prevent_default();
        },
    )?;
    on(canvas_target, "dragover", |event: web_sys::DragEvent| {
        event.prevent_default();
        push(Event::FileHovered);
    })?;
    on(canvas_target, "dragleave", |_event: web_sys::DragEvent| {
        push(Event::FileHoverCancelled);
    })?;
    on(canvas_target, "drop", |event: web_sys::DragEvent| {
        event.prevent_default();
        let Some(files) = event.data_transfer().and_then(|transfer| transfer.files()) else {
            push(Event::FileHoverCancelled);
            return;
        };
        if files.length() == 0 {
            push(Event::FileHoverCancelled);
        }
        for index in 0..files.length() {
            let Some(file) = files.get(index) else {
                continue;
            };
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                    .await
                    .ok()
                    .map(|buffer| js_sys::Uint8Array::new(&buffer).to_vec());
                push(Event::FileDropped(DroppedFile {
                    name: file.name(),
                    path: None,
                    bytes: bytes.map(Arc::from),
                }));
            });
        }
    })?;

    on(agent_target, "keydown", |event: web_sys::KeyboardEvent| {
        let modifiers = modifiers_of(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        let key = key(&event.code());
        if let Some(key) = key {
            push(Event::Key {
                key,
                pressed: true,
                repeat: event.repeat(),
                modifiers,
            });
        }
        let clipboard = modifiers.ctrl && matches!(key, Some(Key::C | Key::V | Key::X));
        let printable = event.key().chars().count() == 1 && !modifiers.ctrl && !modifiers.alt;
        if key.is_some() && !clipboard && !printable {
            event.prevent_default();
        }
    })?;
    on(agent_target, "keyup", |event: web_sys::KeyboardEvent| {
        let modifiers = modifiers_of(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        if let Some(key) = key(&event.code()) {
            push(Event::Key {
                key,
                pressed: false,
                repeat: false,
                modifiers,
            });
        }
    })?;
    on(agent_target, "input", {
        let agent = agent.clone();
        move |event: web_sys::InputEvent| {
            if event.is_composing() {
                return;
            }
            let text = agent.value();
            agent.set_value("");
            if !text.is_empty() {
                push(Event::Text(text));
            }
        }
    })?;
    on(
        agent_target,
        "compositionstart",
        |_event: web_sys::CompositionEvent| {
            push(Event::Ime(ImeEvent::Enabled));
        },
    )?;
    on(
        agent_target,
        "compositionupdate",
        |event: web_sys::CompositionEvent| {
            push(Event::Ime(ImeEvent::Preedit(
                event.data().unwrap_or_default(),
            )));
        },
    )?;
    on(agent_target, "compositionend", {
        let agent = agent.clone();
        move |event: web_sys::CompositionEvent| {
            agent.set_value("");
            let text = event.data().unwrap_or_default();
            if !text.is_empty() {
                push(Event::Text(text));
            }
            push(Event::Ime(ImeEvent::Disabled));
        }
    })?;
    on(agent_target, "paste", |event: web_sys::ClipboardEvent| {
        event.prevent_default();
        if let Some(text) = event
            .clipboard_data()
            .and_then(|data| data.get_data("text").ok())
            .filter(|text| !text.is_empty())
        {
            push(Event::Text(text));
        }
    })?;
    on(agent_target, "focus", |_event: web_sys::FocusEvent| {
        push(Event::Focus(true));
    })?;
    on(agent_target, "blur", |_event: web_sys::FocusEvent| {
        INPUT.with(|input| input.buttons.set(0));
        push(Event::Focus(false));
    })?;

    let document_target: &web_sys::EventTarget = document.as_ref();
    on(document_target, "pointerlockchange", {
        let document = document.clone();
        let canvas: web_sys::Element = canvas.clone().into();
        move |_event: web_sys::Event| {
            let locked = document
                .pointer_lock_element()
                .is_some_and(|element| element == canvas);
            INPUT.with(|input| input.locked.set(locked));
            schedule();
        }
    })?;
    let window_target: &web_sys::EventTarget = window.as_ref();
    on(window_target, "resize", |_event: web_sys::Event| schedule())?;
    let observer = web_sys::ResizeObserver::new(
        Closure::<dyn FnMut()>::new(schedule)
            .into_js_value()
            .unchecked_ref(),
    )
    .map_err(|_| "could not watch the canvas size")?;
    observer.observe(canvas);
    std::mem::forget(observer);
    Ok(())
}

fn touch(event: &web_sys::PointerEvent, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: TouchId {
            device: 0,
            finger: event.pointer_id() as u64,
        },
        phase,
        pos,
        force: Some(event.pressure()),
    }
}

fn pointer_button(button: i16) -> Option<PointerButton> {
    match button {
        0 => Some(PointerButton::Primary),
        1 => Some(PointerButton::Middle),
        2 => Some(PointerButton::Secondary),
        3 => Some(PointerButton::Back),
        4 => Some(PointerButton::Forward),
        _ => None,
    }
}

fn cursor(icon: CursorIcon) -> &'static str {
    match icon {
        CursorIcon::Default => "default",
        CursorIcon::Crosshair => "crosshair",
        CursorIcon::Grab => "grab",
        CursorIcon::Grabbing => "grabbing",
        CursorIcon::NotAllowed => "not-allowed",
        CursorIcon::PointingHand => "pointer",
        CursorIcon::ResizeHorizontal => "ew-resize",
        CursorIcon::ResizeVertical => "ns-resize",
        CursorIcon::ResizeNeSw => "nesw-resize",
        CursorIcon::ResizeNwSe => "nwse-resize",
        CursorIcon::Text => "text",
        CursorIcon::Wait => "wait",
        CursorIcon::None => "none",
        CursorIcon::Move => "move",
        CursorIcon::Progress => "progress",
        CursorIcon::Help => "help",
        CursorIcon::Alias => "alias",
    }
}

fn key(code: &str) -> Option<Key> {
    let key = match code {
        "ArrowDown" => Key::ArrowDown,
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "ArrowUp" => Key::ArrowUp,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "End" => Key::End,
        "Enter" | "NumpadEnter" => Key::Enter,
        "Escape" => Key::Escape,
        "Home" => Key::Home,
        "BracketLeft" => Key::BracketLeft,
        "BracketRight" => Key::BracketRight,
        "Minus" | "NumpadSubtract" => Key::Minus,
        "PageDown" => Key::PageDown,
        "PageUp" => Key::PageUp,
        "Equal" | "NumpadAdd" => Key::Plus,
        "Space" => Key::Space,
        "Tab" => Key::Tab,
        "Digit0" | "Numpad0" => Key::Zero,
        "Digit1" | "Numpad1" => Key::One,
        "Digit2" | "Numpad2" => Key::Two,
        "Digit3" | "Numpad3" => Key::Three,
        "Digit4" | "Numpad4" => Key::Four,
        "Digit5" | "Numpad5" => Key::Five,
        "Digit6" | "Numpad6" => Key::Six,
        "Digit7" | "Numpad7" => Key::Seven,
        "Digit8" | "Numpad8" => Key::Eight,
        "Digit9" | "Numpad9" => Key::Nine,
        "Backquote" => Key::Backtick,
        "KeyA" => Key::A,
        "KeyB" => Key::B,
        "KeyC" => Key::C,
        "KeyD" => Key::D,
        "KeyE" => Key::E,
        "KeyF" => Key::F,
        "KeyG" => Key::G,
        "KeyH" => Key::H,
        "KeyI" => Key::I,
        "KeyJ" => Key::J,
        "KeyK" => Key::K,
        "KeyL" => Key::L,
        "KeyM" => Key::M,
        "KeyN" => Key::N,
        "KeyO" => Key::O,
        "KeyP" => Key::P,
        "KeyQ" => Key::Q,
        "KeyR" => Key::R,
        "KeyS" => Key::S,
        "KeyT" => Key::T,
        "KeyU" => Key::U,
        "KeyV" => Key::V,
        "KeyW" => Key::W,
        "KeyX" => Key::X,
        "KeyY" => Key::Y,
        "KeyZ" => Key::Z,
        "Insert" => Key::Insert,
        "Comma" => Key::Comma,
        "Period" | "NumpadDecimal" => Key::Period,
        "Slash" | "NumpadDivide" => Key::Slash,
        "Backslash" => Key::Backslash,
        "Semicolon" => Key::Semicolon,
        "Quote" => Key::Quote,
        "BrowserBack" => Key::BrowserBack,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        "F13" => Key::F13,
        "F14" => Key::F14,
        "F15" => Key::F15,
        "F16" => Key::F16,
        "F17" => Key::F17,
        "F18" => Key::F18,
        "F19" => Key::F19,
        "F20" => Key::F20,
        "F21" => Key::F21,
        "F22" => Key::F22,
        "F23" => Key::F23,
        "F24" => Key::F24,
        _ => return None,
    };
    Some(key)
}
