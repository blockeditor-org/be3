use std::cell::{Cell, RefCell};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GA_ROOT, GetAncestor,
    GetMessageW, KillTimer, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, RegisterClassW,
    SetTimer, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_HOTKEY, WM_QUIT, WM_SETTINGCHANGE, WM_TIMER, WM_USER, WNDCLASSW, WS_POPUP,
};
use windows::core::w;

use super::desktop::Win32Desktop;
use super::focus::{self, HOTKEY_ID};
use super::{Com, handle, hwnd};
use crate::events::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_DESKTOPSWITCH,
    EVENT_SYSTEM_FOREGROUND, is_any_window_event, is_cursor_event,
};
use crate::{
    Cursor, EndReason, EntryId, Event, FocusError, FocusEvent, FocusMode, Focused, Handle, Hotkey,
    HotkeyError, Launch, Model, Output, Point, Snapshot,
};

const WM_REFRESH: u32 = WM_APP + 1;
const WM_CURSOR: u32 = WM_APP + 2;
const TICK_MS: u32 = 1000;

enum Request {
    Show(EntryId),
    Hide(EntryId),
    Launched(Launch),
    Focus(EntryId, Option<Point>, u64),
    Release,
    Host(Option<Handle>),
    Hotkey(Option<Hotkey>),
}

struct Shared {
    snapshot: Mutex<Arc<Snapshot>>,
    focused: Mutex<Option<Focused>>,
    cursor: Mutex<Option<Cursor>>,
    events: Mutex<Vec<FocusEvent>>,
    requests: Mutex<Vec<Request>>,
    returns: AtomicU64,
    wake: Box<dyn Fn() + Send + Sync>,
}

thread_local! {
    static PENDING: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static POSTED: Cell<bool> = const { Cell::new(false) };
    static CURSOR_POSTED: Cell<bool> = const { Cell::new(false) };
}

struct State<'a> {
    shared: &'a Shared,
    desktop: Win32Desktop,
    model: Model,
    mode: FocusMode,
    host: Option<Handle>,
    hotkey: Option<Hotkey>,
    own_process: u32,
}

pub struct Watcher {
    shared: Arc<Shared>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    pub fn spawn(wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Arc::new(Snapshot::default())),
            focused: Mutex::new(None),
            cursor: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
            returns: AtomicU64::new(0),
            wake: Box::new(wake),
        });
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("be-win32 windows".to_owned())
            .spawn({
                let shared = shared.clone();
                move || run(&shared, &ready)
            })?;
        let thread_id = started
            .recv()
            .map_err(|_| io::Error::other("the windows thread stopped as it started"))??;
        Ok(Self {
            shared,
            thread_id,
            thread: Some(thread),
        })
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.shared.snapshot.lock().expect("snapshot lock").clone()
    }

    pub fn show(&self, id: EntryId) {
        self.request(Request::Show(id));
    }

    pub fn hide(&self, id: EntryId) {
        self.request(Request::Hide(id));
    }

    pub fn launched(&self, launch: Launch) {
        self.request(Request::Launched(launch));
    }

    pub fn focus(&self, id: EntryId, at: Option<Point>) -> Result<(), FocusError> {
        let since = self.shared.returns.load(Ordering::SeqCst);
        let snapshot = self.snapshot();
        let entry = FocusMode::check(&snapshot, id)?;
        focus::raise(&snapshot, entry)?;
        self.request(Request::Focus(id, at, since));
        Ok(())
    }

    pub fn release(&self) {
        self.request(Request::Release);
    }

    pub fn set_host_window(&self, window: Option<Handle>) {
        self.request(Request::Host(window));
    }

    pub fn set_hotkey(&self, hotkey: Option<Hotkey>) -> Result<(), HotkeyError> {
        if let Some(hotkey) = hotkey {
            hotkey.check()?;
        }
        self.request(Request::Hotkey(hotkey));
        Ok(())
    }

    pub fn focused(&self) -> Option<Focused> {
        self.shared.focused.lock().expect("focus lock").clone()
    }

    pub fn cursor(&self) -> Option<Cursor> {
        *self.shared.cursor.lock().expect("cursor lock")
    }

    pub fn take_focus_events(&self) -> Vec<FocusEvent> {
        std::mem::take(&mut *self.shared.events.lock().expect("events lock"))
    }

    // unsafe(ffi)
    fn request(&self, request: Request) {
        self.shared
            .requests
            .lock()
            .expect("requests lock")
            .push(request);
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_REFRESH, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for Watcher {
    // unsafe(ffi)
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// unsafe(ffi)
fn run(shared: &Shared, ready: &mpsc::Sender<io::Result<u32>>) {
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let _com = Com::init();
    let mut message = MSG::default();
    let _ = unsafe { PeekMessageW(&mut message, None, WM_USER, WM_USER, PM_NOREMOVE) };
    let thread_id = unsafe { GetCurrentThreadId() };
    let listener = match listener() {
        Ok(listener) => listener,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let hooks: Vec<HWINEVENTHOOK> = [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_DESKTOPSWITCH),
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_UNCLOAKED),
    ]
    .into_iter()
    .map(|(first, last)| unsafe {
        SetWinEventHook(
            first,
            last,
            None,
            Some(on_win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    })
    .collect();
    let timer = unsafe { SetTimer(None, 0, TICK_MS, None) };
    let _ = ready.send(Ok(thread_id));

    let mut state = State {
        shared,
        desktop: Win32Desktop::new(),
        model: Model::new(),
        mode: FocusMode::new(),
        host: None,
        hotkey: None,
        own_process: unsafe { GetCurrentProcessId() },
    };
    state.refresh(&[]);
    loop {
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match message.message {
            WM_REFRESH => {
                POSTED.set(false);
                let events = PENDING.take();
                let requests = std::mem::take(&mut *shared.requests.lock().expect("requests lock"));
                for request in requests {
                    state.handle(request);
                }
                state.refresh(&events);
            }
            WM_TIMER if message.hwnd.0.is_null() => {
                let mut events = PENDING.take();
                events.push(Event::Tick);
                state.refresh(&events);
            }
            WM_CURSOR => {
                CURSOR_POSTED.set(false);
                state.hold_clip();
                if state.read_cursor() {
                    (shared.wake)();
                }
            }
            WM_HOTKEY if message.hwnd.0.is_null() && message.wParam.0 == HOTKEY_ID as usize => {
                state.events(vec![FocusEvent::Hotkey]);
                state.release(EndReason::Hotkey);
                state.refresh(&PENDING.take());
            }
            _ => {
                unsafe { DispatchMessageW(&message) };
            }
        }
    }
    if state.mode.focused().is_some() {
        focus::apply(crate::Command::Unclip);
    }
    if state.hotkey.is_some() {
        focus::unregister();
    }
    for hook in hooks {
        let _ = unsafe { UnhookWinEvent(hook) };
    }
    if timer != 0 {
        let _ = unsafe { KillTimer(None, timer) };
    }
    let _ = unsafe { DestroyWindow(listener) };
}

impl State<'_> {
    fn handle(&mut self, request: Request) {
        match request {
            Request::Show(id) => self.model.show(id),
            Request::Hide(id) => self.model.hide(id),
            Request::Launched(launch) => self.model.launched(launch),
            Request::Focus(id, at, since) => {
                let snapshot = self.model.snapshot();
                match self.mode.start(&snapshot, id, at, since) {
                    Ok(out) => self.output(out),
                    Err(error) => self.events(vec![FocusEvent::Refused { entry: id, error }]),
                }
            }
            Request::Release => self.release(EndReason::Released),
            Request::Host(window) => self.host = window,
            Request::Hotkey(hotkey) => {
                if self.hotkey.take().is_some() {
                    focus::unregister();
                }
                let event = match hotkey.map(|hotkey| (hotkey, focus::register(hotkey))) {
                    None => FocusEvent::HotkeySet(None),
                    Some((hotkey, Ok(()))) => {
                        self.hotkey = Some(hotkey);
                        FocusEvent::HotkeySet(Some(hotkey))
                    }
                    Some((_, Err(error))) => FocusEvent::HotkeyRefused(error),
                };
                let out = self.mode.set_hotkey(self.hotkey.is_some());
                self.output(out);
                self.events(vec![event]);
            }
        }
    }

    fn release(&mut self, reason: EndReason) {
        let out = self.mode.release(reason);
        focus::apply(crate::Command::Unclip);
        self.output(out);
        if let Some(host) = self.host
            && !focus::bring_forward(hwnd(host))
        {
            self.events(vec![FocusEvent::HostForegroundRefused]);
        }
    }

    fn refresh(&mut self, events: &[Event]) {
        let mut changed = false;
        if self.model.update(&self.desktop, events) {
            *self.shared.snapshot.lock().expect("snapshot lock") = self.model.snapshot();
            changed = true;
            changed |= self.read_cursor();
        }
        let snapshot = self.model.snapshot();
        let foreground = focus::foreground(&snapshot, self.own_process);
        let out = self.mode.step(&snapshot, foreground, events);
        changed |= !out.events.is_empty();
        self.output(out);
        if changed {
            (self.shared.wake)();
        }
    }

    fn hold_clip(&mut self) {
        if let Some(current) = focus::current_clip()
            && let Some(command) = self.mode.reclip(current)
        {
            focus::apply(command);
        }
    }

    fn output(&mut self, out: Output) {
        for command in out.commands {
            focus::apply(command);
        }
        self.shared
            .returns
            .store(self.mode.returns(), Ordering::SeqCst);
        *self.shared.focused.lock().expect("focus lock") = self.mode.focused().cloned();
        if !out.events.is_empty() {
            self.events(out.events);
        }
    }

    fn events(&self, events: Vec<FocusEvent>) {
        self.shared
            .events
            .lock()
            .expect("events lock")
            .extend(events);
        (self.shared.wake)();
    }

    fn read_cursor(&mut self) -> bool {
        let cursor = focus::cursor(&self.model.snapshot());
        let mut current = self.shared.cursor.lock().expect("cursor lock");
        if *current == cursor {
            return false;
        }
        *current = cursor;
        true
    }
}

// unsafe(ffi)
fn queue(event: Event) {
    PENDING.with_borrow_mut(|pending| pending.push(event));
    if !POSTED.replace(true) {
        let _ =
            unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_REFRESH, WPARAM(0), LPARAM(0)) };
    }
}

// unsafe(ffi)
unsafe extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if is_cursor_event(event, object) {
        if !CURSOR_POSTED.replace(true) {
            let _ = unsafe {
                PostThreadMessageW(GetCurrentThreadId(), WM_CURSOR, WPARAM(0), LPARAM(0))
            };
        }
        return;
    }
    let whole = is_any_window_event(event)
        || window.0.is_null()
        || unsafe { GetAncestor(window, GA_ROOT) } == window;
    if !whole {
        return;
    }
    if let Some(event) = Event::from_win_event(event, handle(window), object, child) {
        queue(event);
    }
}

// unsafe(ffi)
unsafe extern "system" fn on_listener_message(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if matches!(message, WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED) {
        queue(Event::DisplaysChanged);
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

// unsafe(ffi)
fn listener() -> io::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }.map_err(super::other)?;
    let class = w!("be-win32 display listener");
    let description = WNDCLASSW {
        lpfnWndProc: Some(on_listener_message),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    unsafe { RegisterClassW(&description) };
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(super::other)
}
