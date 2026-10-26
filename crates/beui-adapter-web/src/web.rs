use std::cell::{Cell, RefCell};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;
use std::time::{Duration, Instant};

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use beui_core::app::ime_mirror::{ImeInput, ImeMirror};
use beui_core::app::{Lifecycle, SafeArea, Setup, Waker};
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest, PickedFile};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_core::geometry::{Pos2, Rect, pos2, vec2};
use beui_core::input::{
    CursorIcon, DroppedFile, FileDrag, ImeArea, ImeEvent, Input, Key, Keystroke, Modifiers, Pen,
    PenPhase, PointerButton, TouchId, TouchPhase,
};
use beui_core::preferences::{Motion, PreferenceLayer, ThemeKind};
use beui_core::renderer::Loaded;
use beui_core::runner::{Adapter, Incoming, Launch, Platform, Running};
use beui_core::window::{Appearance, WindowReports, WindowState};

use crate::lock::Lock;
use crate::wheel::{Pinch, wheel};

type Loading = Pin<Box<dyn Future<Output = Result<Vec<Loaded>, Box<dyn Error>>>>>;
type Load = Box<dyn FnOnce(web_sys::HtmlElement) -> Loading>;

pub struct Web {
    element_id: String,
    load: Load,
}

impl Web {
    pub fn new<F>(
        element_id: impl Into<String>,
        load: impl FnOnce(web_sys::HtmlElement) -> F + 'static,
    ) -> Self
    where
        F: Future<Output = Result<Vec<Loaded>, Box<dyn Error>>> + 'static,
    {
        Self {
            element_id: element_id.into(),
            load: Box::new(move |element| Box::pin(load(element))),
        }
    }
}

impl Adapter for Web {
    fn name(&self) -> &'static str {
        "web"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        let Web { element_id, load } = *self;
        Box::pin(async move { run(&element_id, launch, load).await })
    }
}

struct Pending {
    inputs: RefCell<Vec<Incoming>>,
    scheduled: Cell<bool>,
}

thread_local! {
    static INPUT: Pending = const { Pending {
        inputs: RefCell::new(Vec::new()),
        scheduled: Cell::new(false),
    } };
    static RUNNER: RefCell<Option<Rc<RefCell<Runner>>>> = const { RefCell::new(None) };
    static FRAME: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<Closure<dyn FnMut()>>> = const { RefCell::new(None) };
    static TIMEOUT: Cell<Option<i32>> = const { Cell::new(None) };
    static MIRROR: RefCell<ImeMirror> = RefCell::new(ImeMirror::default());
    static HELD_FIELD: Cell<Option<Rect>> = const { Cell::new(None) };
    static REFOCUSING: Cell<bool> = const { Cell::new(false) };
    static IME_SHOWN: Cell<bool> = const { Cell::new(false) };
    static LEAVING: Cell<bool> = const { Cell::new(false) };
    static CLIPBOARD_KEYS: Cell<bool> = const { Cell::new(false) };
    static PASTE_KINDS: RefCell<Vec<PasteKind>> = const { RefCell::new(Vec::new()) };
    static PINCH: RefCell<Pinch> = RefCell::new(Pinch::default());
    static LOCK: RefCell<Lock> = RefCell::new(Lock::default());
    static TOUCHES: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

const PASTED_NAME: &str = "Pasted Image.png";

fn paste_from(data: &web_sys::DataTransfer) {
    let kinds = PASTE_KINDS.with(|pastes| pastes.borrow().clone());
    for kind in kinds {
        match kind {
            PasteKind::Text => {
                if let Some(text) = data.get_data("text").ok().filter(|text| !text.is_empty()) {
                    push(Input::Paste(Pasted::Text(text)));
                    return;
                }
            }
            PasteKind::Image => {
                let files = data.files();
                let image = files.and_then(|files| {
                    (0..files.length())
                        .filter_map(|index| files.get(index))
                        .find(|file| file.type_().starts_with("image/"))
                });
                if let Some(file) = image {
                    wasm_bindgen_futures::spawn_local(async move {
                        let Ok(buffer) =
                            wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                        else {
                            return;
                        };
                        let name = Some(file.name())
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| PASTED_NAME.to_owned());
                        push(Input::Paste(Pasted::Image(PickedFile {
                            name,
                            data: js_sys::Uint8Array::new(&buffer).to_vec(),
                        })));
                    });
                    return;
                }
            }
        }
    }
}

fn push(input: impl Into<Incoming>) {
    INPUT.with(|pending| pending.inputs.borrow_mut().push(input.into()));
    schedule();
}

fn push_logical(input: Input) {
    push(Incoming::logical(input));
}

fn push_sample(input: Input, sample: &web_sys::Event) {
    push_logical(input);
    push(Incoming::Time(happened(sample)));
}

fn happened(event: &web_sys::Event) -> Instant {
    let now = Instant::now();
    let ago = web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now() - event.time_stamp());
    match ago.is_finite() && ago > 0.0 {
        true => now
            .checked_sub(Duration::from_secs_f64(ago / 1000.0))
            .unwrap_or(now),
        false => now,
    }
}

fn schedule() {
    if INPUT.with(|input| input.scheduled.get()) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let requested = FRAME.with(|frame| {
        frame.borrow().as_ref().is_some_and(|frame| {
            window
                .request_animation_frame(frame.as_ref().unchecked_ref())
                .is_ok()
        })
    });
    INPUT.with(|input| input.scheduled.set(requested));
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
    runner: beui_core::runner::Runner,
    platform: WebPlatform,
    size: (u32, u32),
    ratio: f32,
}

fn copy_png(png: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
    use js_sys::{Array, Function, Object, Reflect, Uint8Array};
    let window = web_sys::window().ok_or("no window")?;
    let options = Object::new();
    Reflect::set(&options, &"type".into(), &"image/png".into())?;
    let blob: Function = Reflect::get(&window, &"Blob".into())?.dyn_into()?;
    let parts = Array::of1(&Uint8Array::from(png));
    let blob = Reflect::construct(&blob, &Array::of2(&parts, &options))?;
    let record = Object::new();
    Reflect::set(&record, &"image/png".into(), &blob)?;
    let item: Function = Reflect::get(&window, &"ClipboardItem".into())?.dyn_into()?;
    let item = Reflect::construct(&item, &Array::of1(&record))?;
    let clipboard = window.navigator().clipboard();
    let write: Function = Reflect::get(&clipboard, &"write".into())?.dyn_into()?;
    write.call1(&clipboard, &Array::of1(&item))?;
    Ok(())
}

struct WebPlatform {
    surface: web_sys::HtmlElement,
    agent: web_sys::HtmlTextAreaElement,
}

impl Platform for WebPlatform {
    fn copy(&mut self, text: String) {
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(&text);
        }
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        if let Err(error) = copy_png(&png) {
            web_sys::console::warn_1(&error);
        }
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let Some(window) = web_sys::window().filter(|_| kinds.contains(&PasteKind::Text)) else {
            return answer.send(None);
        };
        let read = window.navigator().clipboard().read_text();
        wasm_bindgen_futures::spawn_local(async move {
            let text = wasm_bindgen_futures::JsFuture::from(read)
                .await
                .ok()
                .and_then(|text| text.as_string())
                .filter(|text| !text.is_empty());
            answer.send(text.map(Pasted::Text));
        });
    }

    fn shows_keyboard(&self) -> bool {
        true
    }

    fn delivers_paste_shortcut(&self) -> bool {
        true
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        crate::file_picker::open(request, answer);
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        crate::file_saver::open(request, answer);
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        CLIPBOARD_KEYS.set(window.requests.clipboard_keys);
        if window.requests.pastes != shown.requests.pastes {
            PASTE_KINDS.with(|pastes| pastes.borrow_mut().clone_from(&window.requests.pastes));
        }
        if window.requests.cursor != shown.requests.cursor {
            let cursor = match window.requests.cursor {
                CursorIcon::Touch => beui_core::touch_cursor::css(CursorIcon::Touch.name()),
                cursor => cursor.name().to_owned(),
            };
            let _ = self.surface.style().set_property("cursor", &cursor);
        }
        if window.requests.pointer_locked != shown.requests.pointer_locked {
            LOCK.with(|lock| lock.borrow_mut().want(window.requests.pointer_locked));
            match window.requests.pointer_locked {
                true => self.surface.request_pointer_lock(),
                false => {
                    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                        document.exit_pointer_lock();
                    }
                }
            }
        }
        self.mirror(window.requests.ime.as_ref());
        if window.requests.ime != shown.requests.ime {
            self.show_ime(window.requests.ime.as_ref());
        }
        if window.requests.keyboard_toggles != shown.requests.keyboard_toggles {
            self.toggle_keyboard();
        }
        if window.appearance != shown.appearance {
            show_appearance(&window.appearance);
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
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

impl WebPlatform {
    fn toggle_keyboard(&self) {
        let agent: &web_sys::Element = &self.agent;
        let focused = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .is_some_and(|active| &active == agent);
        match focused {
            true => {
                let _ = self.agent.blur();
            }
            false => {
                let _ = self.agent.remove_attribute("inputmode");
                let _ = self.agent.focus();
            }
        }
    }

    fn show_ime(&self, ime: Option<&ImeArea>) {
        let held = ime.filter(|area| !area.keyboard.shown());
        let was_held = HELD_FIELD.replace(held.map(|area| area.rect)).is_some();
        match held {
            Some(_) => {
                let _ = self.agent.set_attribute("inputmode", "none");
            }
            None if was_held => {
                let _ = self.agent.remove_attribute("inputmode");
            }
            None => {}
        }
        if let Some(area) = ime {
            let style = self.agent.style();
            let _ = style.set_property("left", &format!("{}px", area.cursor.min.x));
            let _ = style.set_property("top", &format!("{}px", area.cursor.max.y));
        }
        if ime.is_some() && !IME_SHOWN.replace(true) {
            let agent: &web_sys::Element = &self.agent;
            let focused = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.active_element())
                .is_some_and(|active| &active == agent);
            if !focused {
                let _ = self.agent.focus();
            }
        }
        if ime.is_none() {
            IME_SHOWN.set(false);
        }
    }

    fn mirror(&self, ime: Option<&ImeArea>) {
        let mirrored = MIRROR.with(|mirror| {
            let mut mirror = mirror.borrow_mut();
            let text = ime.and_then(|area| area.text.as_ref());
            mirror
                .sync(text)
                .then(|| (mirror.value().to_owned(), mirror.selection()))
        });
        if let Some((value, (start, end))) = mirrored {
            self.agent.set_value(&value);
            let _ = self.agent.set_selection_range(start, end);
        }
    }
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
        let bounds = self.platform.surface.get_bounding_client_rect();
        let width = ((bounds.width() as f32) * ratio).round().max(1.0) as u32;
        let height = ((bounds.height() as f32) * ratio).round().max(1.0) as u32;
        let reports = self.runner.reports();
        if self.size != (width, height) {
            self.size = (width, height);
            reports.resized(width, height);
        }
        if self.ratio != ratio {
            self.ratio = ratio;
            reports.scaled(ratio);
        }
        for input in INPUT.with(|pending| pending.inputs.take()) {
            self.runner.push(input);
        }
        let Some(frame) = self.runner.frame(&mut self.platform, SafeArea::default()) else {
            if self.runner.exited() {
                self.closed(&window);
            }
            return;
        };
        let presenting = frame.pending && self.runner.present();
        match frame.next(presenting) {
            Some(Duration::ZERO) => schedule(),
            Some(wait) => schedule_after(wait),
            None => {}
        }
    }
}

impl Runner {
    fn closed(&self, window: &web_sys::Window) {
        if let Some(failure) = self.runner.failure() {
            web_sys::console::error_1(&format!("the app stopped after a panic: {failure}").into());
        }
        if !LEAVING.get() {
            let _ = window.close();
        }
    }
}

fn lifecycle(change: Lifecycle) {
    let runner = RUNNER.with(|runner| runner.borrow().clone());
    let Some(runner) = runner else {
        return;
    };
    let Ok(mut runner) = runner.try_borrow_mut() else {
        return;
    };
    runner.runner.lifecycle(change);
    schedule();
}

fn close_requested(event: &web_sys::Event) {
    let runner = RUNNER.with(|runner| runner.borrow().clone());
    let Some(runner) = runner else {
        return;
    };
    let Ok(mut runner) = runner.try_borrow_mut() else {
        return;
    };
    if runner.runner.exited() || runner.runner.os_close() {
        LEAVING.set(true);
        return;
    }
    event.prevent_default();
    let _ = js_sys::Reflect::set(event, &"returnValue".into(), &"".into());
}

struct Wake {
    woken: AtomicBool,
    waker: futures_util::task::AtomicWaker,
}

struct Woken(Arc<Wake>);

impl Future for Woken {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<()> {
        if self.0.woken.swap(false, Ordering::AcqRel) {
            return Poll::Ready(());
        }
        self.0.waker.register(context.waker());
        match self.0.woken.swap(false, Ordering::AcqRel) {
            true => Poll::Ready(()),
            false => Poll::Pending,
        }
    }
}

fn waker() -> Waker {
    let wake = Arc::new(Wake {
        woken: AtomicBool::new(false),
        waker: futures_util::task::AtomicWaker::new(),
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
        wake.waker.wake();
    })
}

pub fn accessibility_tree() -> Option<String> {
    let runner = RUNNER.with(|runner| runner.borrow().clone())?;
    let runner = runner.try_borrow().ok()?;
    runner.runner.accessibility_text()
}

fn show_appearance(appearance: &Appearance) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    if let Some(title) = &appearance.title {
        document.set_title(title);
    }
    if let Some(body) = document.body() {
        let [red, green, blue, alpha] = appearance.clear_color.to_array();
        let color = format!("rgba({red},{green},{blue},{})", f32::from(alpha) / 255.0);
        let _ = body.style().set_property("background", &color);
    }
}

pub fn request_frame() {
    schedule();
}

async fn run(element_id: &str, launch: Launch, load: Load) -> Result<(), Box<dyn Error>> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let document = window
        .document()
        .ok_or("no browser document is available")?;
    let surface = document
        .get_element_by_id(element_id)
        .ok_or_else(|| format!("no element has the id {element_id}"))?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| format!("the element {element_id} is not an html element"))?;
    show_appearance(&Appearance::default());
    let _ = surface.style().set_property("touch-action", "none");
    let agent = text_agent(&document)?;
    let loaded = load(surface.clone()).await?;

    let mut runner = beui_core::runner::Runner::new(launch);
    runner.start(loaded, Setup::new(waker()))?;
    let reports = runner.reports().clone();
    let runner = Runner {
        runner,
        platform: WebPlatform {
            surface: surface.clone(),
            agent: agent.clone(),
        },
        size: (0, 0),
        ratio: 0.0,
    };
    RUNNER.with(|slot| *slot.borrow_mut() = Some(Rc::new(RefCell::new(runner))));
    FRAME.with(|frame| *frame.borrow_mut() = Some(Closure::new(run_frame)));
    TIMER.with(|timer| {
        *timer.borrow_mut() = Some(Closure::new(|| {
            TIMEOUT.set(None);
            schedule();
        }))
    });
    listen(&window, &document, &surface, &agent, &reports)?;
    let pointer = window.match_media("(pointer: fine)").ok().flatten();
    if pointer.is_some_and(|query| query.matches()) {
        let _ = agent.focus();
    }
    watch_preferences(&window, &reports)?;
    schedule();
    Ok(())
}

fn watch_preferences(
    window: &web_sys::Window,
    reports: &WindowReports,
) -> Result<(), Box<dyn Error>> {
    let query = |media: &str| window.match_media(media).ok().flatten();
    let (Some(dark), Some(light), Some(reduce)) = (
        query("(prefers-color-scheme: dark)"),
        query("(prefers-color-scheme: light)"),
        query("(prefers-reduced-motion: reduce)"),
    ) else {
        return Ok(());
    };
    let report = {
        let (dark, light, reduce, reports) =
            (dark.clone(), light.clone(), reduce.clone(), reports.clone());
        move || {
            reports.system_preferences(PreferenceLayer {
                theme: match (dark.matches(), light.matches()) {
                    (true, _) => Some(ThemeKind::Dark),
                    (false, true) => Some(ThemeKind::Light),
                    (false, false) => None,
                },
                motion: reduce.matches().then_some(Motion::Instant),
                canvas_touch: None,
            });
        }
    };
    report();
    for list in [dark, light, reduce] {
        let report = report.clone();
        on(&list, "change", move |_: web_sys::Event| {
            report();
            schedule();
        })?;
    }
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
    agent.set_attribute("autocorrect", "on").ok();
    agent.set_attribute("spellcheck", "true").ok();
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

fn position(surface: &web_sys::HtmlElement, event: &web_sys::Event) -> Pos2 {
    within(&surface.get_bounding_client_rect(), event)
}

fn within(bounds: &web_sys::DomRect, event: &web_sys::Event) -> Pos2 {
    let (x, y) = (
        client(event, "clientX").unwrap_or_default(),
        client(event, "clientY").unwrap_or_default(),
    );
    pos2((x - bounds.left()) as f32, (y - bounds.top()) as f32)
}

fn client(event: &web_sys::Event, axis: &str) -> Option<f64> {
    js_sys::Reflect::get(event, &axis.into()).ok()?.as_f64()
}

fn pointer_list(event: &web_sys::PointerEvent, method: &str) -> Vec<web_sys::PointerEvent> {
    let list = js_sys::Reflect::get(event, &method.into())
        .ok()
        .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
        .and_then(|function| function.call0(event).ok());
    let Some(list) = list else {
        return Vec::new();
    };
    js_sys::Array::from(&list)
        .iter()
        .filter_map(|sample| sample.dyn_into().ok())
        .collect()
}

fn touching(id: i32, down: bool) {
    TOUCHES.with(|touches| {
        let mut touches = touches.borrow_mut();
        touches.retain(|touch| *touch != id);
        if down {
            touches.push(id);
        }
    });
}

thread_local! {
    static APPLE: bool = web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .is_some_and(|platform| platform.starts_with("Mac") || platform.starts_with("iP"));
}

fn apple() -> bool {
    APPLE.with(|apple| *apple)
}

fn modifiers_of(alt: bool, ctrl: bool, meta: bool, shift: bool) -> Modifiers {
    let modifiers = Modifiers::with_meta(alt, ctrl, shift, meta, apple());
    push(Input::Modifiers(modifiers));
    modifiers
}

fn listen(
    window: &web_sys::Window,
    document: &web_sys::Document,
    surface: &web_sys::HtmlElement,
    agent: &web_sys::HtmlTextAreaElement,
    reports: &WindowReports,
) -> Result<(), Box<dyn Error>> {
    let surface_target: &web_sys::EventTarget = surface.as_ref();
    let agent_target: &web_sys::EventTarget = agent.as_ref();

    on(surface_target, "pointerdown", {
        let surface = surface.clone();
        let agent = agent.clone();
        move |event: web_sys::PointerEvent| {
            event.prevent_default();
            let _ = agent.focus();
            let _ = surface.set_pointer_capture(event.pointer_id());
            modifiers_of(
                event.alt_key(),
                event.ctrl_key(),
                event.meta_key(),
                event.shift_key(),
            );
            let pos = position(&surface, &event);
            if event.pointer_type() == "touch" {
                touching(event.pointer_id(), true);
            }
            if event.pointer_type() == "touch"
                && HELD_FIELD
                    .with(|held| held.get())
                    .is_some_and(|rect| rect.contains(pos))
            {
                HELD_FIELD.set(None);
                let _ = agent.remove_attribute("inputmode");
                REFOCUSING.set(true);
                let _ = agent.blur();
                let _ = agent.focus();
                REFOCUSING.set(false);
            }
            if event.pointer_type() == "touch" {
                push_sample(touch(&event, TouchPhase::Start, pos), &event);
                return;
            }
            if event.pointer_type() == "pen" {
                if let Some(button) = pen_button(event.button()) {
                    push_sample(pen(&event, PenPhase::Down(button), pos), &event);
                }
                return;
            }
            if let Some(button) = pointer_button(event.button()) {
                if LOCK.with(|lock| lock.borrow_mut().press(button)) {
                    surface.request_pointer_lock();
                    return;
                }
                push_sample(button_event(pos, button, true), &event);
            }
        }
    })?;
    on(surface_target, "pointermove", {
        let surface = surface.clone();
        move |event: web_sys::PointerEvent| {
            if LOCK.with(|lock| lock.borrow().held()) {
                push_logical(Input::PointerMotion(vec2(
                    event.movement_x() as f32,
                    event.movement_y() as f32,
                )));
                return;
            }
            let bounds = surface.get_bounding_client_rect();
            let mut samples = pointer_list(&event, "getCoalescedEvents");
            if samples.is_empty() {
                samples.push(event.clone());
            }
            for sample in &samples {
                let pos = within(&bounds, sample);
                let moved = match event.pointer_type().as_str() {
                    "touch" => touch(sample, TouchPhase::Move, pos),
                    "pen" => pen(sample, PenPhase::Move, pos),
                    _ => Input::PointerMoved(pos),
                };
                push_sample(moved, sample);
            }
            if event.buttons() != 0
                && let Some(predicted) = pointer_list(&event, "getPredictedEvents").last()
            {
                push_logical(Input::Predicted(within(&bounds, predicted)));
            }
        }
    })?;
    for (name, cancelled) in [("pointerup", false), ("pointercancel", true)] {
        on(surface_target, name, {
            let surface = surface.clone();
            move |event: web_sys::PointerEvent| {
                let _ = surface.release_pointer_capture(event.pointer_id());
                modifiers_of(
                    event.alt_key(),
                    event.ctrl_key(),
                    event.meta_key(),
                    event.shift_key(),
                );
                let pos = position(&surface, &event);
                if event.pointer_type() == "touch" {
                    touching(event.pointer_id(), false);
                    let phase = match cancelled {
                        true => TouchPhase::Cancel,
                        false => TouchPhase::End,
                    };
                    push_sample(touch(&event, phase, pos), &event);
                    return;
                }
                if event.pointer_type() == "pen" {
                    let phase = match (cancelled, pen_button(event.button())) {
                        (false, Some(button)) => PenPhase::Up(button),
                        (false, None) => return,
                        (true, _) => PenPhase::Gone,
                    };
                    push_sample(pen(&event, phase, pos), &event);
                    return;
                }
                if let Some(button) = pointer_button(event.button()) {
                    if LOCK.with(|lock| lock.borrow_mut().release(button)) {
                        return;
                    }
                    push_sample(button_event(pos, button, false), &event);
                }
            }
        })?;
    }
    on(surface_target, "pointerleave", {
        let surface = surface.clone();
        move |event: web_sys::PointerEvent| match event.pointer_type().as_str() {
            "touch" => {}
            "pen" => {
                let pos = position(&surface, &event);
                push_logical(pen(&event, PenPhase::Gone, pos));
            }
            _ => push(Input::PointerLeft),
        }
    })?;
    on(surface_target, "wheel", |event: web_sys::WheelEvent| {
        event.prevent_default();
        let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
        let input =
            PINCH.with(|pinch| wheel(delta, event.delta_mode(), event.ctrl_key(), &pinch.borrow()));
        if let Some(input) = input {
            push_logical(input);
        }
    })?;
    on(surface_target, "gesturestart", {
        let surface = surface.clone();
        move |event: web_sys::Event| {
            event.prevent_default();
            PINCH.with(|pinch| pinch.borrow_mut().start());
            if TOUCHES.with(|touches| touches.borrow().is_empty()) {
                push_logical(Input::PointerMoved(position(&surface, &event)));
            }
        }
    })?;
    on(surface_target, "gesturechange", |event: web_sys::Event| {
        event.prevent_default();
        let scale = client(&event, "scale").unwrap_or(1.0) as f32;
        let touching = TOUCHES.with(|touches| !touches.borrow().is_empty());
        if let Some(factor) = PINCH.with(|pinch| pinch.borrow_mut().change(scale, touching)) {
            push_logical(Input::Zoom(factor));
        }
    })?;
    on(surface_target, "gestureend", |event: web_sys::Event| {
        event.prevent_default();
        PINCH.with(|pinch| pinch.borrow_mut().end());
    })?;
    on(
        surface_target,
        "contextmenu",
        |event: web_sys::MouseEvent| {
            event.prevent_default();
        },
    )?;
    on(surface_target, "dragover", |event: web_sys::DragEvent| {
        event.prevent_default();
        push(FileDrag::Hovered);
    })?;
    on(surface_target, "dragleave", |_event: web_sys::DragEvent| {
        push(FileDrag::Cancelled);
    })?;
    on(surface_target, "drop", |event: web_sys::DragEvent| {
        event.prevent_default();
        let Some(files) = event.data_transfer().and_then(|transfer| transfer.files()) else {
            push(FileDrag::Cancelled);
            return;
        };
        if files.length() == 0 {
            push(FileDrag::Cancelled);
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
                push(FileDrag::Dropped(DroppedFile {
                    name: file.name(),
                    path: None,
                    bytes: bytes.map(Arc::from),
                }));
            });
        }
    })?;

    on(agent_target, "keydown", |event: web_sys::KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        let modifiers = modifiers_of(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        let stroke = keystroke(&event, true);
        let key = stroke.key;
        push(Input::Key(stroke));
        if crate::keys::prevents_default(key, &event.key(), modifiers, CLIPBOARD_KEYS.get()) {
            event.prevent_default();
        }
    })?;
    on(agent_target, "keyup", |event: web_sys::KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        modifiers_of(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        );
        push(Input::Key(keystroke(&event, false)));
    })?;
    on(agent_target, "input", {
        let agent = agent.clone();
        move |event: web_sys::InputEvent| {
            let value = agent.value();
            let input = ImeInput {
                value: &value,
                selection: agent_selection(&agent),
                composing: event.is_composing(),
                replacement: event.input_type() == "insertReplacementText",
            };
            mirror_events(|mirror, events| mirror.input(input, events));
        }
    })?;
    on(
        agent_target,
        "compositionstart",
        |_event: web_sys::CompositionEvent| {
            MIRROR.with(|mirror| mirror.borrow_mut().start_composition());
            push(Input::Ime(ImeEvent::Enabled));
        },
    )?;
    on(
        agent_target,
        "compositionend",
        |_event: web_sys::CompositionEvent| {
            mirror_events(|mirror, events| {
                mirror.end_composition(events);
                events.push(Input::Ime(ImeEvent::Disabled));
            });
        },
    )?;
    let selection_target: &web_sys::EventTarget = document.as_ref();
    on(selection_target, "selectionchange", {
        let agent = agent.clone();
        let document = document.clone();
        move |_event: web_sys::Event| {
            let element: &web_sys::Element = agent.as_ref();
            let focused = document
                .active_element()
                .is_some_and(|active| active == *element);
            if focused {
                let selection = agent_selection(&agent);
                mirror_events(|mirror, events| mirror.select(selection, events));
            }
        }
    })?;
    on(agent_target, "paste", |event: web_sys::ClipboardEvent| {
        event.prevent_default();
        if let Some(data) = event.clipboard_data() {
            paste_from(&data);
        }
    })?;
    on(agent_target, "focus", |_event: web_sys::FocusEvent| {
        if !REFOCUSING.get() {
            push(Input::Focus(true));
        }
    })?;
    on(agent_target, "blur", |_event: web_sys::FocusEvent| {
        if REFOCUSING.get() {
            return;
        }
        push(Input::Focus(false));
    })?;

    let document_target: &web_sys::EventTarget = document.as_ref();
    on(document_target, "pointerlockchange", {
        let document = document.clone();
        let surface: web_sys::Element = surface.clone().into();
        move |_event: web_sys::Event| {
            let locked = document
                .pointer_lock_element()
                .is_some_and(|element| element == surface);
            match LOCK.with(|lock| lock.borrow_mut().changed(locked)) {
                Some(lost) => push(lost),
                None => schedule(),
            }
        }
    })?;
    on(
        document_target,
        "pointerlockerror",
        |_event: web_sys::Event| {
            if let Some(refused) = LOCK.with(|lock| lock.borrow_mut().refused()) {
                push(refused);
            }
        },
    )?;
    on(document_target, "fullscreenchange", {
        let document = document.clone();
        let reports = reports.clone();
        move |_event: web_sys::Event| {
            reports.fullscreen(document.fullscreen_element().is_some());
            schedule();
        }
    })?;
    let window_target: &web_sys::EventTarget = window.as_ref();
    on(window_target, "resize", |_event: web_sys::Event| schedule())?;
    on(window_target, "blur", |_event: web_sys::FocusEvent| {
        push(Input::PointersCancelled);
    })?;
    on(window_target, "focus", |_event: web_sys::FocusEvent| {
        lifecycle(Lifecycle::Focused);
    })?;
    on(window_target, "online", |_event: web_sys::Event| {
        lifecycle(Lifecycle::NetworkChanged);
    })?;
    on(window_target, "pagehide", |_event: web_sys::Event| {
        lifecycle(Lifecycle::Background);
    })?;
    on(document_target, "visibilitychange", {
        let document = document.clone();
        move |_event: web_sys::Event| {
            lifecycle(match document.hidden() {
                true => Lifecycle::Background,
                false => Lifecycle::Foreground,
            });
        }
    })?;
    on(window_target, "beforeunload", |event: web_sys::Event| {
        close_requested(&event);
    })?;
    let observer = web_sys::ResizeObserver::new(
        Closure::<dyn FnMut()>::new(schedule)
            .into_js_value()
            .unchecked_ref(),
    )
    .map_err(|_| "could not watch the element's size")?;
    observer.observe(surface);
    std::mem::forget(observer);
    Ok(())
}

fn agent_selection(agent: &web_sys::HtmlTextAreaElement) -> (u32, u32) {
    let start = agent.selection_start().ok().flatten().unwrap_or(0);
    let end = agent.selection_end().ok().flatten().unwrap_or(start);
    (start, end)
}

fn mirror_events(change: impl FnOnce(&mut ImeMirror, &mut Vec<Input>)) {
    let mut events = Vec::new();
    MIRROR.with(|mirror| change(&mut mirror.borrow_mut(), &mut events));
    for event in events {
        push(event);
    }
}

fn touch(event: &web_sys::PointerEvent, phase: TouchPhase, pos: Pos2) -> Input {
    Input::Touch {
        id: TouchId {
            device: 0,
            finger: event.pointer_id() as u64,
        },
        phase,
        pos,
        force: Some(event.pressure()),
    }
}

fn pen(event: &web_sys::PointerEvent, phase: PenPhase, pos: Pos2) -> Input {
    Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure: event.pressure(),
            tilt: vec2(event.tilt_x() as f32, event.tilt_y() as f32),
            eraser: event.buttons() & PEN_ERASER != 0 || event.button() == 5,
        },
    }
}

const PEN_ERASER: u16 = 32;

fn pen_button(button: i16) -> Option<PointerButton> {
    match button {
        5 => Some(PointerButton::Primary),
        button => pointer_button(button),
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

fn key(event: &web_sys::KeyboardEvent) -> Option<Key> {
    let physical = Key::evdev_from_code(&event.code()).and_then(Key::from_evdev);
    Key::from_value(&event.key(), physical, apple())
}

fn keystroke(event: &web_sys::KeyboardEvent, pressed: bool) -> Keystroke {
    Keystroke {
        key: key(event),
        pressed,
        repeat: pressed && event.repeat(),
        evdev: Key::evdev_from_code(&event.code()),
        text: None,
    }
}

fn button_event(pos: Pos2, button: PointerButton, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button,
        pressed,
    }
}
