#![cfg(target_os = "android")]

mod clipboard;

use std::error::Error;
use std::ffi::CString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use accesskit_android::InjectingAdapter;
use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::refs::Global;
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::vm::JavaVM;
use jni::{EnvUnowned, jni_sig, jni_str};
use ndk::asset::AssetManager;
use ndk::native_window::NativeWindow;
use raw_window_handle as rwh;

use beui_core::app::{Lifecycle, SafeArea, Setup, Waker};
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::file_picker::{FilePick, FilePickAnswer, FilePickId, FilePickRequest, PickedFile};
use beui_core::file_saver::{FileSave, FileSaveAnswer, FileSaveId, FileSaveRequest};
use beui_core::geometry::{Vec2, pos2, vec2};
use beui_core::input::{
    BackEdge, BackGesture, ImeArea, ImeEvent, ImeText, Input, Key, Keyboard, Keystroke, Modifiers,
    Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
use beui_core::preferences::{Motion, PreferenceLayer, ThemeKind};
use beui_core::window::{SystemBars, WindowState};
use beui_runner::renderer::{Loaded, WindowHandle};
use beui_runner::{Incoming, Launch, Platform, PlatformServices, Running};
use clipboard::Clipboard;

const SURFACE_RELEASE_TIMEOUT: Duration = Duration::from_secs(2);

const META_SHIFT_ON: jint = 0x1;
const META_ALT_ON: jint = 0x2;
const META_CTRL_ON: jint = 0x1000;
const META_META_ON: jint = 0x10000;

// unsafe(ffi)
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

// unsafe(send-sync)
unsafe impl Send for AppInner {}
// unsafe(send-sync)
unsafe impl Sync for AppInner {}

// unsafe(ffi)
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
    Preferences(PreferenceLayer),
    Focus(bool),
    Lifecycle(Lifecycle),
    Touch {
        device: jint,
        id: jint,
        phase: TouchPhase,
        x: f32,
        y: f32,
        force: f32,
        at: Instant,
    },
    Pen {
        phase: PenPhase,
        x: f32,
        y: f32,
        pen: Pen,
        at: Instant,
    },
    Scroll(Vec2),
    Key {
        key: Option<Key>,
        pressed: bool,
        repeat: bool,
        evdev: Option<u32>,
        modifiers: Modifiers,
        text: Option<String>,
    },
    Ime {
        event: ImeEvent,
        serial: u64,
    },
    Back(BackGesture),
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
static PICKS: Mutex<Vec<FilePickAnswer>> = Mutex::new(Vec::new());
static SAVES: Mutex<Vec<FileSaveAnswer>> = Mutex::new(Vec::new());

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

type Load = Box<dyn FnOnce(Arc<dyn WindowHandle>) -> Result<Loaded, Box<dyn Error>>>;

pub struct Android {
    load: Load,
}

impl Android {
    pub fn new(
        load: impl FnOnce(Arc<dyn WindowHandle>) -> Result<Loaded, Box<dyn Error>> + 'static,
    ) -> Self {
        Self {
            load: Box::new(load),
        }
    }
}

impl Platform for Android {
    fn name(&self) -> &'static str {
        "android"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(launch, self.load) })
    }
}

fn run(launch: Launch, load: Load) -> Result<(), Box<dyn Error>> {
    let receiver = shared()
        .receiver
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or("the app is already running")?;
    let mut runner = Runner {
        runner: beui_runner::Runner::new(launch),
        platform: AndroidPlatform {
            vm: JavaVM::singleton()?,
            view: None,
            accessibility: None,
            clipboard: Clipboard::new(),
            ime_serial: 0,
            sent_ime: None,
            keyboard_shown: std::cell::Cell::new(false),
            light_bars: None,
        },
        load: Some(load),
        window: None,
        safe_area: SafeArea::default(),
        next_update: None,
        redraw: false,
        error: None,
    };
    runner.run(&receiver);
    match runner.error {
        Some(error) => Err(error.into()),
        None => runner.runner.finish(),
    }
}

struct Runner {
    runner: beui_runner::Runner,
    platform: AndroidPlatform,
    load: Option<Load>,
    window: Option<NativeWindow>,
    safe_area: SafeArea,
    next_update: Option<Instant>,
    redraw: bool,
    error: Option<String>,
}

struct AndroidPlatform {
    vm: JavaVM,
    view: Option<Arc<Global<JObject<'static>>>>,
    accessibility: Option<InjectingAdapter>,
    clipboard: Clipboard,
    ime_serial: u64,
    sent_ime: Option<(Option<ImeText>, u64)>,
    keyboard_shown: std::cell::Cell<bool>,
    light_bars: Option<bool>,
}

impl PlatformServices for AndroidPlatform {
    fn copy(&mut self, text: String) {
        self.clipboard.set(text);
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let text = kinds
            .contains(&PasteKind::Text)
            .then(|| self.clipboard.get())
            .flatten();
        answer.send(text.map(Pasted::Text));
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        AndroidPlatform::pick_file(self, request, answer);
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        AndroidPlatform::save_file(self, request, answer);
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        if window.requests.handles_back != shown.requests.handles_back {
            self.set_back_handled(window.requests.handles_back);
        }
        self.sync_ime(window.requests.ime.as_ref());
        let asked = shown.requests.ime.as_ref().map(|area| area.keyboard);
        let wanted = window.requests.ime.as_ref().map(|area| area.keyboard);
        match wanted {
            Some(Keyboard::Shown { .. }) if wanted != asked => self.set_keyboard(true),
            Some(Keyboard::Shown { .. }) => {}
            _ if asked.is_some_and(Keyboard::shown) => self.set_keyboard(false),
            _ => {}
        }
        if window.requests.keyboard_toggles != shown.requests.keyboard_toggles {
            self.set_keyboard(!self.keyboard_shown.get());
        }
    }

    fn shows_keyboard(&self) -> bool {
        true
    }

    fn publish_accessibility(&mut self, tree: &mut dyn FnMut() -> TreeUpdate) {
        if let Some(accessibility) = &mut self.accessibility {
            accessibility.update_if_active(tree);
        }
    }
}

// unsafe(ffi)
impl AndroidPlatform {
    fn sync_ime(&mut self, ime: Option<&ImeArea>) {
        let sent = (ime.and_then(|area| area.text.clone()), self.ime_serial);
        if self.sent_ime.as_ref() != Some(&sent) {
            self.set_ime_text(sent.0.as_ref(), sent.1);
            self.sent_ime = Some(sent);
        }
    }

    fn set_keyboard(&self, shown: bool) {
        self.keyboard_shown.set(shown);
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

    fn set_ime_text(&self, text: Option<&ImeText>, serial: u64) {
        let Some(view) = &self.view else {
            return;
        };
        let offset = |index: usize| jlong::try_from(index).unwrap_or(jlong::MAX);
        let _ = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                let (start, value, selection, composing) = match text {
                    Some(text) => (
                        offset(text.start),
                        env.new_string(&text.text)?,
                        (offset(text.selection.start), offset(text.selection.end)),
                        text.composing
                            .as_ref()
                            .map_or((-1, -1), |range| (offset(range.start), offset(range.end))),
                    ),
                    None => (0, env.new_string("")?, (0, 0), (-1, -1)),
                };
                env.call_method(
                    view.as_obj(),
                    jni_str!("setImeText"),
                    jni_sig!("(JLjava/lang/String;JJJJJ)V"),
                    &[
                        JValue::Long(start),
                        JValue::Object(&value),
                        JValue::Long(selection.0),
                        JValue::Long(selection.1),
                        JValue::Long(composing.0),
                        JValue::Long(composing.1),
                        JValue::Long(jlong::try_from(serial).unwrap_or(jlong::MAX)),
                    ],
                )?;
                Ok(())
            });
    }

    fn sync_bars(&mut self, bars: SystemBars) {
        let light = bars == SystemBars::Light;
        if self.light_bars == Some(light) {
            return;
        }
        let Some(view) = &self.view else {
            return;
        };
        self.light_bars = Some(light);
        let _ = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                env.call_method(
                    view.as_obj(),
                    jni_str!("setLightBars"),
                    jni_sig!("(Z)V"),
                    &[JValue::Bool(light)],
                )?;
                Ok(())
            });
    }

    fn set_back_handled(&self, handled: bool) {
        let Some(view) = &self.view else {
            return;
        };
        let _ = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                env.call_method(
                    view.as_obj(),
                    jni_str!("setBackHandled"),
                    jni_sig!("(Z)V"),
                    &[JValue::Bool(handled)],
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

    fn pick_file(&self, request: FilePickRequest, answer: FilePickAnswer) {
        let Some(view) = &self.view else {
            answer.send(Err("Choosing a file is unavailable right now".into()));
            return;
        };
        let id = request.id;
        PICKS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(answer);
        let started = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                let mime_types = env.new_string(request.filter.mime_types.join(","))?;
                env.call_method(
                    view.as_obj(),
                    jni_str!("pickFile"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[
                        JValue::Long(id.0.cast_signed()),
                        JValue::Object(&mime_types),
                    ],
                )?;
                Ok(())
            });
        if let Err(error) = started
            && let Some(answer) = take_pick(id)
        {
            answer.send(Err(format!("Could not open a file picker: {error}")));
        }
    }

    fn save_file(&self, request: FileSaveRequest, answer: FileSaveAnswer) {
        let Some(view) = &self.view else {
            answer.send(FileSave::Failed(
                "Saving a file is unavailable right now".into(),
            ));
            return;
        };
        let id = request.id;
        SAVES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(answer);
        let file = request.file;
        let started = self
            .vm
            .attach_current_thread(|env| -> Result<(), JniError> {
                let name = env.new_string(&file.name)?;
                let mime_type = env.new_string(&file.mime_type)?;
                let data = env.byte_array_from_slice(&file.data)?;
                env.call_method(
                    view.as_obj(),
                    jni_str!("saveFile"),
                    jni_sig!("(JLjava/lang/String;Ljava/lang/String;[B)V"),
                    &[
                        JValue::Long(id.0.cast_signed()),
                        JValue::Object(&name),
                        JValue::Object(&mime_type),
                        JValue::Object(&data),
                    ],
                )?;
                Ok(())
            });
        if let Err(error) = started
            && let Some(answer) = take_save(id)
        {
            answer.send(FileSave::Failed(format!(
                "Could not open a save dialog: {error}"
            )));
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
}

impl Runner {
    fn run(&mut self, receiver: &Receiver<Message>) {
        while !self.runner.exited() {
            let due = self.redraw
                || self.runner.has_events()
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
        self.runner.exit();
    }

    fn fail(&mut self, error: impl ToString) {
        self.error = Some(error.to_string());
        self.exit();
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::View(view) => {
                let platform = &mut self.platform;
                platform.accessibility = None;
                platform.view = Some(view);
                platform.attach_accessibility();
                platform.ime_serial = 0;
                platform.sent_ime = None;
                platform.light_bars = None;
                let window = self.runner.window();
                platform.sync_ime(window.requests.ime.as_ref());
                if window
                    .requests
                    .ime
                    .as_ref()
                    .is_some_and(|area| area.keyboard.shown())
                {
                    platform.set_keyboard(true);
                }
                if window.requests.handles_back {
                    platform.set_back_handled(true);
                }
            }
            Message::Surface {
                window,
                width,
                height,
                density,
            } => self.surface(window, width, height, density),
            Message::SurfaceGone(done) => {
                if let Some(rendering) = self.runner.rendering() {
                    rendering.detach();
                }
                self.window = None;
                let _ = done.send(());
            }
            Message::Insets(area) => {
                self.safe_area = area;
                self.redraw = true;
            }
            Message::Preferences(preferences) => {
                self.runner.reports().system_preferences(preferences);
                self.redraw = true;
            }
            Message::Focus(focused) => {
                self.runner.push(Input::Focus(focused));
                if focused {
                    self.runner.lifecycle(Lifecycle::Focused);
                }
            }
            Message::Lifecycle(change) => {
                self.runner.lifecycle(change);
                self.redraw = true;
            }
            Message::Touch {
                device,
                id,
                phase,
                x,
                y,
                force,
                at,
            } => {
                let pos = pos2(x, y);
                self.runner.push(Input::Touch {
                    id: TouchId {
                        device: u64::from(device.cast_unsigned()),
                        finger: u64::from(id.cast_unsigned()),
                    },
                    phase,
                    pos,
                    force: Some(force),
                });
                self.runner.push(Incoming::Time(at));
            }
            Message::Pen {
                phase,
                x,
                y,
                pen,
                at,
            } => {
                self.runner.push(Input::Pen {
                    phase,
                    pos: pos2(x, y),
                    pen,
                });
                self.runner.push(Incoming::Time(at));
            }
            Message::Scroll(delta) => self.runner.push(Input::Scroll(ScrollDelta::Lines(delta))),
            Message::Key {
                key,
                pressed,
                repeat,
                evdev,
                modifiers,
                text,
            } => {
                self.runner.push(Input::Modifiers(modifiers));
                self.runner.push(Input::Key(Keystroke {
                    key,
                    pressed,
                    repeat,
                    evdev,
                    text,
                }));
            }
            Message::Ime { event, serial } => {
                self.platform.ime_serial = self.platform.ime_serial.max(serial);
                self.runner.push(Input::Ime(event));
            }
            Message::Back(gesture) => self.runner.push(Input::Back(gesture)),
            Message::InitialTreeRequested => {
                self.runner.accessibility_requested();
                self.redraw = true;
            }
            Message::Action(request) => {
                self.runner.accessibility_action(request);
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

    fn surface(&mut self, window: NativeWindow, width: u32, height: u32, density: f32) {
        self.runner.reports().scaled(density);
        self.runner.reports().resized(width, height);
        self.redraw = true;
        if !self.runner.started()
            && let Err(error) = self.start(&window)
        {
            return self.fail(error);
        }
        let Some(rendering) = self.runner.rendering() else {
            return;
        };
        if rendering.attached() && self.window.as_ref() == Some(&window) {
            return;
        }
        rendering.detach();
        self.window = None;
        let handle = Arc::new(SurfaceWindow(window.clone()));
        if let Err(error) = rendering.attach(handle, width, height) {
            return self.fail(error);
        }
        self.window = Some(window);
    }

    fn start(&mut self, window: &NativeWindow) -> Result<(), Box<dyn Error>> {
        let load = self
            .load
            .take()
            .ok_or("the renderers were already loaded")?;
        let loaded = load(Arc::new(SurfaceWindow(window.clone())))?;
        let sender = shared().sender.clone();
        let setup = Setup::new(Waker::new(move || {
            let _ = sender.send(Message::Wake);
        }));
        self.runner.start(loaded, setup)
    }

    fn frame(&mut self) {
        let redraw = std::mem::take(&mut self.redraw);
        let pending = self.update();
        if (pending || redraw) && self.runner.present() {
            self.redraw = true;
        }
    }

    fn update(&mut self) -> bool {
        let Some(frame) = self.runner.frame(&mut self.platform, self.safe_area) else {
            self.next_update = None;
            if self.runner.exited() {
                self.platform.finish_activity();
            }
            return false;
        };
        self.next_update = frame
            .wait()
            .and_then(|wait| Instant::now().checked_add(wait));
        self.platform
            .sync_bars(self.runner.window().appearance.system_bars);
        frame.pending
    }
}

struct SurfaceWindow(NativeWindow);

// unsafe(ffi)
impl rwh::HasWindowHandle for SurfaceWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let handle = rwh::AndroidNdkWindowHandle::new(self.0.ptr().cast());
        Ok(unsafe { rwh::WindowHandle::borrow_raw(rwh::RawWindowHandle::AndroidNdk(handle)) })
    }
}

impl rwh::HasDisplayHandle for SurfaceWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::android())
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
        ctrl: meta & META_CTRL_ON != 0,
        shift: meta & META_SHIFT_ON != 0,
        logo: meta & META_META_ON != 0,
    }
}

fn unicode(character: jint) -> Option<char> {
    u32::try_from(character)
        .ok()
        .filter(|character| *character > 0)
        .and_then(char::from_u32)
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
        82 => Key::ContextMenu,
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
        57 | 58 => Key::Alt,
        59 | 60 => Key::Shift,
        113 | 114 => Key::Ctrl,
        117 | 118 => Key::Logo,
        _ => return None,
    };
    Some(key)
}

fn back_gesture(phase: jint, progress: f32, edge: jint) -> Option<BackGesture> {
    let edge = match edge {
        0 => BackEdge::Left,
        1 => BackEdge::Right,
        _ => BackEdge::None,
    };
    Some(match phase {
        0 => BackGesture::Started { edge },
        1 => BackGesture::Progressed(progress),
        2 => BackGesture::Cancelled,
        3 => BackGesture::Invoked,
        _ => return None,
    })
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

// unsafe(ffi)
fn forward_stdio_to_logcat() {
    let mut pipe = [0; 2];
    let reader = unsafe {
        if libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) != 0 {
            return;
        }
        libc::dup2(pipe[1], libc::STDOUT_FILENO);
        libc::dup2(pipe[1], libc::STDERR_FILENO);
        libc::close(pipe[1]);
        File::from_raw_fd(pipe[0])
    };
    let _ = std::thread::Builder::new()
        .name("stdio-to-logcat".to_owned())
        .spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                let Ok(text) = CString::new(line) else {
                    continue;
                };
                unsafe {
                    ndk_sys::__android_log_write(
                        ndk_sys::android_LogPriority::ANDROID_LOG_INFO.0 as _,
                        c"RustStdoutStderr".as_ptr(),
                        text.as_ptr(),
                    );
                }
            }
        });
}

fn count(value: jint) -> u32 {
    value.max(0).cast_unsigned()
}

// unsafe(ffi)
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
        forward_stdio_to_logcat();
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

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeDestroy<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    finishing: jboolean,
) {
    send(Message::Destroy { finishing });
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeLifecycle<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    change: jint,
) {
    let change = match change {
        0 => Lifecycle::Background,
        1 => Lifecycle::Foreground,
        _ => Lifecycle::NetworkChanged,
    };
    send(Message::Lifecycle(change));
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeFocus<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    focused: jboolean,
) {
    send(Message::Focus(focused));
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeBack<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    phase: jint,
    progress: jfloat,
    edge: jint,
) {
    if let Some(gesture) = back_gesture(phase, progress, edge) {
        send(Message::Back(gesture));
    }
}

// unsafe(ffi)
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

// unsafe(ffi)
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

// unsafe(ffi)
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

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativePreferences<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    dark: jboolean,
    light: jboolean,
    reduce_motion: jboolean,
) {
    send(Message::Preferences(PreferenceLayer {
        theme: match (dark, light) {
            (true, _) => Some(ThemeKind::Dark),
            (false, true) => Some(ThemeKind::Light),
            (false, false) => None,
        },
        motion: reduce_motion.then_some(Motion::Instant),
        canvas_touch: None,
    }));
}

// unsafe(ffi)
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
    ago_nanos: jlong,
) {
    if let Some(phase) = touch_phase(phase) {
        send(Message::Touch {
            device,
            id,
            phase,
            x,
            y,
            force: pressure,
            at: monotonic(ago_nanos),
        });
    }
}

fn monotonic(ago_nanos: jlong) -> Instant {
    let now = Instant::now();
    let ago = Duration::from_nanos(u64::try_from(ago_nanos).unwrap_or(0));
    now.checked_sub(ago).unwrap_or(now)
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativePen<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    phase: jint,
    button: jint,
    x: jfloat,
    y: jfloat,
    pressure: jfloat,
    tilt_x: jfloat,
    tilt_y: jfloat,
    eraser: jboolean,
    ago_nanos: jlong,
) {
    let button = match button {
        1 => PointerButton::Secondary,
        _ => PointerButton::Primary,
    };
    let phase = match phase {
        0 => PenPhase::Move,
        1 => PenPhase::Down(button),
        2 => PenPhase::Up(button),
        3 => PenPhase::Gone,
        _ => return,
    };
    send(Message::Pen {
        phase,
        x,
        y,
        pen: Pen {
            pressure,
            tilt: vec2(tilt_x, tilt_y),
            eraser,
        },
        at: monotonic(ago_nanos),
    });
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeScroll<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    x: jfloat,
    y: jfloat,
) {
    send(Message::Scroll(vec2(x, y)));
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeKey<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
    code: jint,
    scan_code: jint,
    pressed: jboolean,
    repeat: jint,
    meta: jint,
    character: jint,
    layout_character: jint,
) -> jboolean {
    let character = unicode(character);
    let key = Key::from_typed(unicode(layout_character), key(code));
    let types = character.is_some_and(|character| !character.is_control());
    let handled = key.is_some() || types || is_modifier(code);
    let text = character.map(String::from);
    if handled {
        send(Message::Key {
            key,
            pressed,
            repeat: repeat > 0,
            evdev: u32::try_from(scan_code).ok().filter(|evdev| *evdev > 0),
            modifiers: modifiers(meta),
            text,
        });
    }
    handled
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeIme<'local>(
    env: EnvUnowned<'local>,
    _: JClass<'local>,
    kind: jint,
    first: jlong,
    second: jlong,
    text: JString<'local>,
    serial: jlong,
) {
    let index = |value: jlong| usize::try_from(value).unwrap_or(0);
    let string = || match text.is_null() {
        true => String::new(),
        false => read(env, &text),
    };
    let event = match kind {
        0 => ImeEvent::SetComposingText(string()),
        1 => ImeEvent::CommitText(string()),
        2 => ImeEvent::FinishComposing,
        3 => ImeEvent::SetComposingRegion(index(first)..index(second)),
        4 => ImeEvent::ReplaceText {
            range: index(first)..index(second),
            text: string(),
        },
        5 => ImeEvent::DeleteSurrounding {
            before: index(first),
            after: index(second),
        },
        6 => ImeEvent::SetSelection {
            anchor: index(first),
            focus: index(second),
        },
        _ => return,
    };
    send(Message::Ime {
        event,
        serial: u64::try_from(serial).unwrap_or(0),
    });
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeFilePicked<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    id: jlong,
    name: JString<'local>,
    data: JByteArray<'local>,
    error: JString<'local>,
) {
    let pick = env
        .with_env(|env| -> Result<FilePick, JniError> {
            if !error.is_null() {
                return Ok(Err(error.try_to_string(env)?));
            }
            if data.is_null() {
                return Ok(Ok(None));
            }
            let name = match name.is_null() {
                true => String::new(),
                false => name.try_to_string(env)?,
            };
            let data = env.convert_byte_array(&data)?;
            Ok(Ok(Some(PickedFile { name, data })))
        })
        .into_outcome();
    let pick = match pick {
        jni::Outcome::Ok(pick) => pick,
        jni::Outcome::Err(_) | jni::Outcome::Panic(_) => {
            Err("The chosen file could not be read".to_owned())
        }
    };
    if let Some(answer) = take_pick(FilePickId(id.cast_unsigned())) {
        answer.send(pick);
    }
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_beui_BeuiView_nativeFileSaved<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    id: jlong,
    saved: jboolean,
    error: JString<'local>,
) {
    let save = env
        .with_env(|env| -> Result<FileSave, JniError> {
            if !error.is_null() {
                return Ok(FileSave::Failed(error.try_to_string(env)?));
            }
            Ok(match saved {
                true => FileSave::Saved,
                false => FileSave::Cancelled,
            })
        })
        .into_outcome();
    let save = match save {
        jni::Outcome::Ok(save) => save,
        jni::Outcome::Err(_) | jni::Outcome::Panic(_) => {
            FileSave::Failed("The file could not be saved".to_owned())
        }
    };
    if let Some(answer) = take_save(FileSaveId(id.cast_unsigned())) {
        answer.send(save);
    }
}

fn take_save(id: FileSaveId) -> Option<FileSaveAnswer> {
    let mut saves = SAVES.lock().unwrap_or_else(PoisonError::into_inner);
    let index = saves.iter().position(|answer| answer.id() == id)?;
    Some(saves.swap_remove(index))
}

fn take_pick(id: FilePickId) -> Option<FilePickAnswer> {
    let mut picks = PICKS.lock().unwrap_or_else(PoisonError::into_inner);
    let index = picks.iter().position(|answer| answer.id() == id)?;
    Some(picks.swap_remove(index))
}
