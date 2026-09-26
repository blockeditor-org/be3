use std::error::Error;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use accesskit_android::InjectingAdapter;
use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{JClass, JObject, JString, JValue};
use jni::refs::Global;
use jni::sys::{jboolean, jfloat, jint};
use jni::vm::JavaVM;
use jni::{EnvUnowned, jni_sig, jni_str};
use ndk::asset::AssetManager;
use ndk::native_window::NativeWindow;

use super::accessibility_dump::AccessibilityDump;
use super::clipboard::Clipboard;
use super::present::{Gpu, Presented, Target, create_gpu, safe_rect};
use super::{App, RunOptions, SafeArea, Setup, Waker};
use crate::context::Context;
use crate::geometry::{Pos2, Vec2, pos2, vec2};
use crate::input::{Event, ImeArea, ImeEvent, Key, Modifiers, RawInput, TouchId, TouchPhase};

const LINE_HEIGHT: f32 = 40.0;
const SURFACE_RELEASE_TIMEOUT: Duration = Duration::from_secs(2);

const META_SHIFT_ON: jint = 0x1;
const META_ALT_ON: jint = 0x2;
const META_CTRL_ON: jint = 0x1000;
const META_META_ON: jint = 0x10000;

unsafe extern "Rust" {
    fn android_main(app: AndroidApp);
}

#[derive(Clone)]
pub struct AndroidApp(Arc<AppInner>);

struct AppInner {
    _assets: Global<JObject<'static>>,
    manager: NonNull<ndk_sys::AAssetManager>,
    files: PathBuf,
}

unsafe impl Send for AppInner {}
unsafe impl Sync for AppInner {}

impl AndroidApp {
    pub fn asset_manager(&self) -> AssetManager {
        unsafe { AssetManager::from_ptr(self.0.manager) }
    }

    pub fn internal_data_path(&self) -> Option<PathBuf> {
        Some(self.0.files.clone())
    }
}

enum Message {
    View(Arc<Global<JObject<'static>>>),
    Surface {
        window: NativeWindow,
        width: u32,
        height: u32,
        density: f32,
    },
    SurfaceGone(SyncSender<()>),
    Insets(SafeArea),
    Focus(bool),
    Touch {
        device: jint,
        id: jint,
        phase: TouchPhase,
        x: f32,
        y: f32,
        force: f32,
    },
    Scroll(Vec2),
    Key {
        key: Option<Key>,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
        text: Option<String>,
    },
    Compose(String),
    Commit {
        text: String,
        composing: bool,
    },
    Recompose {
        by: i32,
        delete: u32,
        text: String,
    },
    Delete {
        before: u32,
        after: u32,
    },
    Move(i32),
    InitialTreeRequested,
    Action(ActionRequest),
    Wake,
    Destroy {
        finishing: bool,
    },
}

struct Shared {
    sender: Sender<Message>,
    receiver: Mutex<Option<Receiver<Message>>>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);
static ACTIVITY: Mutex<Option<Global<JObject<'static>>>> = Mutex::new(None);

fn shared() -> &'static Shared {
    SHARED.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        Shared {
            sender,
            receiver: Mutex::new(Some(receiver)),
        }
    })
}

fn send(message: Message) -> bool {
    shared().sender.send(message).is_ok()
}

pub fn run(title: impl Into<String>, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::new(title), app)
}

pub fn run_with(options: RunOptions, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    let receiver = shared()
        .receiver
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or("the app is already running")?;
    let accessibility_dump = options
        .accessibility_dump
        .clone()
        .map(AccessibilityDump::new);
    let mut runner = Runner {
        vm: JavaVM::singleton()?,
        options,
        app: Box::new(app),
        context: Context::new(),
        gpu: None,
        target: None,
        window: None,
        density: 1.0,
        view: None,
        accessibility: None,
        accessibility_active: false,
        accessibility_dump,
        events: Vec::new(),
        modifiers: Modifiers::NONE,
        safe_area: SafeArea::default(),
        ime: None,
        preedit: String::new(),
        restart_input: false,
        tapped_at: None,
        next_update: None,
        redraw: false,
        clipboard: Clipboard::new(),
        exiting: false,
        error: None,
    };
    runner.run(&receiver);
    match runner.error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

struct Runner {
    vm: JavaVM,
    options: RunOptions,
    app: Box<dyn App>,
    context: Context,
    gpu: Option<Gpu>,
    target: Option<Target>,
    window: Option<NativeWindow>,
    density: f32,
    view: Option<Arc<Global<JObject<'static>>>>,
    accessibility: Option<InjectingAdapter>,
    accessibility_active: bool,
    accessibility_dump: Option<AccessibilityDump>,
    events: Vec<Event>,
    modifiers: Modifiers,
    safe_area: SafeArea,
    ime: Option<ImeArea>,
    preedit: String,
    restart_input: bool,
    tapped_at: Option<Pos2>,
    next_update: Option<Instant>,
    redraw: bool,
    clipboard: Clipboard,
    exiting: bool,
    error: Option<String>,
}

impl Runner {
    fn run(&mut self, receiver: &Receiver<Message>) {
        while !self.exiting {
            let due = self.redraw
                || !self.events.is_empty()
                || self
                    .next_update
                    .is_some_and(|deadline| deadline <= Instant::now());
            let message = if due {
                match receiver.try_recv() {
                    Ok(message) => Some(message),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                }
            } else {
                match self.next_update {
                    Some(deadline) => {
                        match receiver
                            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        {
                            Ok(message) => Some(message),
                            Err(RecvTimeoutError::Timeout) => None,
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    None => match receiver.recv() {
                        Ok(message) => Some(message),
                        Err(_) => break,
                    },
                }
            };
            match message {
                Some(message) => self.handle(message),
                None => self.frame(),
            }
        }
        self.exit();
    }

    fn exit(&mut self) {
        if !self.exiting {
            self.exiting = true;
            self.app.exiting();
        }
    }

    fn fail(&mut self, error: impl ToString) {
        self.error = Some(error.to_string());
        self.exit();
    }

    fn scale_factor(&self) -> f32 {
        self.context
            .simulated_pixels_per_point()
            .unwrap_or(self.density * self.context.zoom_factor())
    }

    fn logical(&self, x: f32, y: f32) -> Pos2 {
        let scale = self.scale_factor();
        pos2(x / scale, y / scale)
    }

    fn set_keyboard(&self, shown: bool) {
        let Some(view) = &self.view else {
            return;
        };
        let _ = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                env.call_method(
                    view.as_obj(),
                    jni_str!("setKeyboard"),
                    jni_sig!("(Z)V"),
                    &[JValue::Bool(shown)],
                )?;
                Ok(())
            });
    }

    fn finish_activity(&self) {
        let Some(view) = &self.view else {
            return;
        };
        let _ = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                env.call_method(
                    view.as_obj(),
                    jni_str!("finishActivity"),
                    jni_sig!("()V"),
                    &[],
                )?;
                Ok(())
            });
    }

    fn press(&mut self, key: Key, times: u32) {
        for _ in 0..times {
            super::press(key, &mut self.events);
        }
    }

    fn move_by(&mut self, by: i32) {
        let key = if by < 0 {
            Key::ArrowLeft
        } else {
            Key::ArrowRight
        };
        self.press(key, by.unsigned_abs());
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::View(view) => {
                self.accessibility = None;
                self.view = Some(view);
                self.attach_accessibility();
                if self.ime.is_some() {
                    self.set_keyboard(true);
                }
            }
            Message::Surface {
                window,
                width,
                height,
                density,
            } => self.surface(window, width, height, density),
            Message::SurfaceGone(done) => {
                if let Some(target) = &mut self.target {
                    target.detach();
                }
                self.window = None;
                let _ = done.send(());
            }
            Message::Insets(area) => {
                self.safe_area = area;
                self.redraw = true;
            }
            Message::Focus(focused) => {
                if !focused && self.modifiers != Modifiers::NONE {
                    self.modifiers = Modifiers::NONE;
                    self.events.push(Event::Modifiers(self.modifiers));
                }
                self.events.push(Event::Focus(focused));
            }
            Message::Touch {
                device,
                id,
                phase,
                x,
                y,
                force,
            } => {
                let pos = self.logical(x, y);
                if phase == TouchPhase::Start && !self.preedit.is_empty() {
                    let preedit = std::mem::take(&mut self.preedit);
                    self.events.push(Event::Ime(ImeEvent::Commit(preedit)));
                    self.restart_input = true;
                }
                if phase == TouchPhase::End {
                    self.tapped_at = Some(pos);
                }
                self.events.push(Event::Touch {
                    id: TouchId {
                        device: u64::from(device.cast_unsigned()),
                        finger: u64::from(id.cast_unsigned()),
                    },
                    phase,
                    pos,
                    force: Some(force),
                });
            }
            Message::Scroll(delta) => self.events.push(Event::Scroll(delta * LINE_HEIGHT)),
            Message::Key {
                key,
                pressed,
                repeat,
                modifiers,
                text,
            } => {
                if modifiers != self.modifiers {
                    self.modifiers = modifiers;
                    self.events.push(Event::Modifiers(modifiers));
                }
                if pressed
                    && key == Some(Key::V)
                    && modifiers.ctrl
                    && !modifiers.alt
                    && let Some(text) = self.clipboard.get()
                {
                    self.events.push(Event::Text(text));
                }
                if let Some(key) = key {
                    self.events.push(Event::Key {
                        key,
                        pressed,
                        repeat,
                        modifiers,
                    });
                }
                if pressed
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && let Some(text) = text
                {
                    self.events.push(Event::Text(text));
                }
            }
            Message::Compose(text) => {
                self.preedit.clone_from(&text);
                self.events.push(Event::Ime(ImeEvent::Preedit(text)));
            }
            Message::Commit { text, composing } => {
                if composing || !self.preedit.is_empty() {
                    self.preedit.clear();
                    self.events
                        .push(Event::Ime(ImeEvent::Commit(String::new())));
                }
                super::typed("", &text, &mut self.events);
            }
            Message::Recompose { by, delete, text } => {
                self.move_by(by);
                self.press(Key::Backspace, delete);
                self.preedit.clone_from(&text);
                self.events.push(Event::Ime(ImeEvent::Preedit(text)));
            }
            Message::Delete { before, after } => {
                self.press(Key::Backspace, before);
                self.press(Key::Delete, after);
            }
            Message::Move(by) => self.move_by(by),
            Message::InitialTreeRequested => {
                self.accessibility_active = true;
                self.context.reset_accessibility();
                self.redraw = true;
            }
            Message::Action(request) => {
                self.context.accessibility_action(request);
                self.redraw = true;
            }
            Message::Wake => self.redraw = true,
            Message::Destroy { finishing } => {
                if finishing {
                    self.exit();
                }
            }
        }
    }

    fn attach_accessibility(&mut self) {
        let Some(view) = &self.view else {
            return;
        };
        let sender = shared().sender.clone();
        let adapter = self.vm.attach_current_thread(|env| -> Result<_, JniError> {
            let mut env = unsafe { accesskit_android::jni::JNIEnv::from_raw(env.get_raw().cast()) }
                .map_err(|_| JniError::NullPtr("JNIEnv"))?;
            let host =
                unsafe { accesskit_android::jni::objects::JObject::from_raw(view.as_raw().cast()) };
            Ok(InjectingAdapter::new(
                &mut env,
                &host,
                Activation(sender.clone()),
                Actions(sender),
            ))
        });
        match adapter {
            Ok(adapter) => self.accessibility = Some(adapter),
            Err(error) => eprintln!("beui: accessibility is unavailable: {error}"),
        }
    }

    fn surface(&mut self, window: NativeWindow, width: u32, height: u32, density: f32) {
        self.density = density;
        self.redraw = true;
        if self.gpu.is_none()
            && let Err(error) = self.start(&window)
        {
            return self.fail(error);
        }
        let (Some(gpu), Some(target)) = (&self.gpu, &mut self.target) else {
            return;
        };
        if target.attached() && self.window.as_ref() == Some(&window) {
            target.resize(gpu, width, height);
            return;
        }
        target.detach();
        self.window = None;
        let surface = match create_surface(&gpu.instance, &window) {
            Ok(surface) => surface,
            Err(error) => return self.fail(error),
        };
        self.window = Some(window);
        if let Err(error) = target.attach(gpu, surface, width, height) {
            self.fail(error);
        }
    }

    fn start(&mut self, window: &NativeWindow) -> Result<(), Box<dyn Error>> {
        let instance = wgpu::Instance::default();
        let probe = create_surface(&instance, window)?;
        let gpu = pollster::block_on(create_gpu(
            instance,
            &probe,
            &self.context,
            self.options.open_device.clone(),
        ))?;
        drop(probe);
        let sender = shared().sender.clone();
        let setup = Setup {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            format: gpu.format,
            waker: Waker::new(move || {
                let _ = sender.send(Message::Wake);
            }),
        };
        self.target = Some(Target::new(gpu.format));
        self.gpu = Some(gpu);
        self.app.setup(&setup);
        Ok(())
    }

    fn frame(&mut self) {
        let redraw = std::mem::take(&mut self.redraw);
        let pending = self.update();
        if !(pending || redraw) {
            return;
        }
        let (Some(target), Some(gpu)) = (&mut self.target, &mut self.gpu) else {
            return;
        };
        if let Presented::Again = target.present(gpu, self.app.clear_color()) {
            self.redraw = true;
        }
    }

    fn update(&mut self) -> bool {
        let (Some(target), Some(gpu)) = (&mut self.target, &mut self.gpu) else {
            return false;
        };
        let Some(physical) = target.physical() else {
            self.events.clear();
            self.next_update = None;
            return false;
        };

        self.context.set_pixels_per_point(self.density);
        self.context.set_test_ids_published(false);
        let scale = self.context.pixels_per_point();
        let screen = vec2(physical.x / scale, physical.y / scale);

        let raw = RawInput {
            events: super::next_batch(&mut self.events),
        };
        let app = &mut self.app;
        let safe_area = self.safe_area;
        let output = self.context.run(raw, |context| {
            app.update(context, safe_rect(screen, safe_area, scale));
        });
        if self.accessibility_active
            && let Some(accessibility) = &mut self.accessibility
        {
            accessibility
                .update_if_active(|| output.accessibility_tree(&self.options.title, screen));
        }
        if let Some(dump) = &mut self.accessibility_dump {
            dump.update(output.accessibility_tree(&self.options.title, screen));
        }

        if let Some(text) = &output.copied_text {
            self.clipboard.set(text.clone());
        }
        if output.paste_requested
            && let Some(text) = self.clipboard.get()
        {
            self.events.push(Event::Text(text));
            self.redraw = true;
        }
        let pending = target.prepare(gpu, &output, scale, self.app.clear_color());
        self.next_update = Instant::now().checked_add(output.repaint_after);

        let asked = self.ime.is_some();
        let restart = std::mem::take(&mut self.restart_input);
        let tapped_field = self
            .tapped_at
            .take()
            .is_some_and(|tap| asked && output.ime.is_some_and(|area| area.rect.contains(tap)));
        if output.ime.is_some() != asked {
            self.preedit.clear();
            self.set_keyboard(output.ime.is_some());
        } else if output.ime.is_some() && (restart || tapped_field) {
            self.set_keyboard(true);
        }
        self.ime = output.ime;
        if output.close_requested {
            self.finish_activity();
            self.exit();
        }
        pending
    }
}

fn create_surface(
    instance: &wgpu::Instance,
    window: &NativeWindow,
) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
    let handle = wgpu::rwh::AndroidNdkWindowHandle::new(window.ptr().cast());
    unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Android(
                wgpu::rwh::AndroidDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::AndroidNdk(handle),
        })
    }
}

struct Activation(Sender<Message>);

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let _ = self.0.send(Message::InitialTreeRequested);
        None
    }
}

struct Actions(Sender<Message>);

impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        let _ = self.0.send(Message::Action(request));
    }
}

fn modifiers(meta: jint) -> Modifiers {
    Modifiers {
        alt: meta & META_ALT_ON != 0,
        ctrl: meta & (META_CTRL_ON | META_META_ON) != 0,
        shift: meta & META_SHIFT_ON != 0,
    }
}

fn is_modifier(code: jint) -> bool {
    matches!(code, 57..=60 | 113 | 114 | 117 | 118)
}

fn key(code: jint) -> Option<Key> {
    let key = match code {
        7 | 144 => Key::Zero,
        8 | 145 => Key::One,
        9 | 146 => Key::Two,
        10 | 147 => Key::Three,
        11 | 148 => Key::Four,
        12 | 149 => Key::Five,
        13 | 150 => Key::Six,
        14 | 151 => Key::Seven,
        15 | 152 => Key::Eight,
        16 | 153 => Key::Nine,
        19 => Key::ArrowUp,
        20 => Key::ArrowDown,
        21 => Key::ArrowLeft,
        22 => Key::ArrowRight,
        29 => Key::A,
        30 => Key::B,
        31 => Key::C,
        32 => Key::D,
        33 => Key::E,
        34 => Key::F,
        35 => Key::G,
        36 => Key::H,
        37 => Key::I,
        38 => Key::J,
        39 => Key::K,
        40 => Key::L,
        41 => Key::M,
        42 => Key::N,
        43 => Key::O,
        44 => Key::P,
        45 => Key::Q,
        46 => Key::R,
        47 => Key::S,
        48 => Key::T,
        49 => Key::U,
        50 => Key::V,
        51 => Key::W,
        52 => Key::X,
        53 => Key::Y,
        54 => Key::Z,
        55 => Key::Comma,
        56 | 158 => Key::Period,
        61 => Key::Tab,
        62 => Key::Space,
        66 | 160 => Key::Enter,
        67 => Key::Backspace,
        68 => Key::Backtick,
        69 | 156 => Key::Minus,
        70 | 81 | 157 => Key::Plus,
        71 => Key::BracketLeft,
        72 => Key::BracketRight,
        73 => Key::Backslash,
        74 => Key::Semicolon,
        75 => Key::Quote,
        76 | 154 => Key::Slash,
        92 => Key::PageUp,
        93 => Key::PageDown,
        111 => Key::Escape,
        112 => Key::Delete,
        122 => Key::Home,
        123 => Key::End,
        124 => Key::Insert,
        131 => Key::F1,
        132 => Key::F2,
        133 => Key::F3,
        134 => Key::F4,
        135 => Key::F5,
        136 => Key::F6,
        137 => Key::F7,
        138 => Key::F8,
        139 => Key::F9,
        140 => Key::F10,
        141 => Key::F11,
        142 => Key::F12,
        _ => return None,
    };
    Some(key)
}

fn touch_phase(phase: jint) -> Option<TouchPhase> {
    Some(match phase {
        0 => TouchPhase::Start,
        1 => TouchPhase::Move,
        2 => TouchPhase::End,
        3 => TouchPhase::Cancel,
        _ => return None,
    })
}

fn read(mut env: EnvUnowned<'_>, text: &JString<'_>) -> String {
    env.with_env(|env| text.try_to_string(env))
        .resolve::<LogErrorAndDefault>()
}

fn count(value: jint) -> u32 {
    value.max(0).cast_unsigned()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeCreate<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    activity: JObject<'local>,
    view: JObject<'local>,
    assets: JObject<'local>,
    files: JString<'local>,
) {
    env.with_env(|env| -> Result<(), JniError> {
        let vm = env.get_java_vm()?;
        let activity = env.new_global_ref(&activity)?;
        let view = Arc::new(env.new_global_ref(&view)?);
        {
            let mut current = ACTIVITY.lock().unwrap_or_else(PoisonError::into_inner);
            unsafe {
                if current.is_some() {
                    ndk_context::release_android_context();
                }
                ndk_context::initialize_android_context(
                    vm.get_raw().cast(),
                    activity.as_raw().cast(),
                );
            }
            *current = Some(activity);
        }
        send(Message::View(view));
        if STARTED.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let manager = unsafe {
            ndk_sys::AAssetManager_fromJava(env.get_raw().cast(), assets.as_raw().cast())
        };
        let manager = NonNull::new(manager).ok_or(JniError::NullPtr("AAssetManager"))?;
        let app = AndroidApp(Arc::new(AppInner {
            _assets: env.new_global_ref(&assets)?,
            manager,
            files: PathBuf::from(files.try_to_string(env)?),
        }));
        std::thread::Builder::new()
            .name("beui".to_owned())
            .spawn(move || {
                let _ = vm.attach_current_thread(|_| Ok::<_, JniError>(()));
                unsafe { android_main(app) };
            })
            .expect("the app's thread starts");
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeDestroy<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    finishing: jboolean,
) {
    send(Message::Destroy { finishing });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeFocus<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    focused: jboolean,
) {
    send(Message::Focus(focused));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeSurfaceChanged<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    surface: JObject<'local>,
    width: jint,
    height: jint,
    density: jfloat,
) {
    env.with_env(|env| -> Result<(), JniError> {
        let window =
            unsafe { NativeWindow::from_surface(env.get_raw().cast(), surface.as_raw().cast()) };
        if let Some(window) = window {
            send(Message::Surface {
                window,
                width: count(width),
                height: count(height),
                density,
            });
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeSurfaceDestroyed<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
) {
    let (done, released) = mpsc::sync_channel(1);
    if send(Message::SurfaceGone(done)) {
        let _ = released.recv_timeout(SURFACE_RELEASE_TIMEOUT);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeInsets<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    left: jint,
    top: jint,
    right: jint,
    bottom: jint,
) {
    send(Message::Insets(SafeArea {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    }));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeTouch<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    device: jint,
    id: jint,
    phase: jint,
    x: jfloat,
    y: jfloat,
    pressure: jfloat,
) {
    if let Some(phase) = touch_phase(phase) {
        send(Message::Touch {
            device,
            id,
            phase,
            x,
            y,
            force: pressure,
        });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeScroll<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    x: jfloat,
    y: jfloat,
) {
    send(Message::Scroll(vec2(x, y)));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeKey<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    code: jint,
    pressed: jboolean,
    repeat: jint,
    meta: jint,
    character: jint,
) -> jboolean {
    let key = key(code);
    let text = u32::try_from(character)
        .ok()
        .filter(|character| *character > 0)
        .and_then(char::from_u32)
        .filter(|character| !character.is_control())
        .map(String::from);
    let handled = key.is_some() || text.is_some() || is_modifier(code);
    if handled {
        send(Message::Key {
            key,
            pressed,
            repeat: repeat > 0,
            modifiers: modifiers(meta),
            text,
        });
    }
    handled
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeCompose<'local>(
    env: EnvUnowned<'local>,
    _: JClass<'local>,
    text: JString<'local>,
) {
    send(Message::Compose(read(env, &text)));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeCommit<'local>(
    env: EnvUnowned<'local>,
    _: JClass<'local>,
    text: JString<'local>,
    composing: jboolean,
) {
    send(Message::Commit {
        text: read(env, &text),
        composing,
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeRecompose<'local>(
    env: EnvUnowned<'local>,
    _: JClass<'local>,
    by: jint,
    delete: jint,
    text: JString<'local>,
) {
    send(Message::Recompose {
        by,
        delete: count(delete),
        text: read(env, &text),
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeDelete<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    before: jint,
    after: jint,
) {
    send(Message::Delete {
        before: count(before),
        after: count(after),
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeMove<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    by: jint,
) {
    send(Message::Move(by));
}
