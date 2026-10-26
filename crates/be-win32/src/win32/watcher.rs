use std::cell::{Cell, RefCell};
use std::io;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GA_ROOT, GetAncestor, GetMessageW,
    KillTimer, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, RegisterClassW, SetTimer, WINDOW_EX_STYLE,
    WINEVENT_OUTOFCONTEXT, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_QUIT, WM_SETTINGCHANGE, WM_TIMER,
    WM_USER, WNDCLASSW, WS_POPUP,
};
use windows::core::w;

use super::desktop::Win32Desktop;
use super::{Com, handle};
use crate::events::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_DESKTOPSWITCH,
    EVENT_SYSTEM_FOREGROUND,
};
use crate::{EntryId, Event, Launch, Model, Snapshot};

const WM_REFRESH: u32 = WM_APP + 1;
const TICK_MS: u32 = 1000;

enum Request {
    Show(EntryId),
    Hide(EntryId),
    Launched(Launch),
}

struct Shared {
    snapshot: Mutex<Arc<Snapshot>>,
    requests: Mutex<Vec<Request>>,
    wake: Box<dyn Fn() + Send + Sync>,
}

thread_local! {
    static PENDING: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static POSTED: Cell<bool> = const { Cell::new(false) };
}

/// The desktop's windows and monitors, watched on a thread of their own:
/// `SetWinEventHook` for windows, a hidden window for display changes, and a
/// tick for whether windows respond. Each change publishes a new
/// [`Snapshot`] and calls `wake`, from that thread.
pub struct Watcher {
    shared: Arc<Shared>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    pub fn spawn(wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Arc::new(Snapshot::default())),
            requests: Mutex::new(Vec::new()),
            wake: Box::new(wake),
        });
        let (ready, started) = mpsc::channel();
        let thread = std::thread::Builder::new().name("be-win32 windows".to_owned()).spawn({
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

    /// Records a launch from [`super::launch`] or [`super::exec`], so the
    /// windows it opens carry its origin.
    pub fn launched(&self, launch: Launch) {
        self.request(Request::Launched(launch));
    }

    // unsafe(ffi)
    fn request(&self, request: Request) {
        self.shared.requests.lock().expect("requests lock").push(request);
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
    .map(|(first, last)| unsafe { SetWinEventHook(first, last, None, Some(on_win_event), 0, 0, WINEVENT_OUTOFCONTEXT) })
    .collect();
    let timer = unsafe { SetTimer(None, 0, TICK_MS, None) };
    let _ = ready.send(Ok(thread_id));

    let desktop = Win32Desktop::new();
    let mut model = Model::new();
    publish(shared, &mut model, &desktop, &[]);
    loop {
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match message.message {
            WM_REFRESH => {
                POSTED.set(false);
                let events = PENDING.take();
                for request in std::mem::take(&mut *shared.requests.lock().expect("requests lock")) {
                    match request {
                        Request::Show(id) => model.show(id),
                        Request::Hide(id) => model.hide(id),
                        Request::Launched(launch) => model.launched(launch),
                    }
                }
                publish(shared, &mut model, &desktop, &events);
            }
            WM_TIMER if message.hwnd.0.is_null() => {
                let mut events = PENDING.take();
                events.push(Event::Tick);
                publish(shared, &mut model, &desktop, &events);
            }
            _ => {
                unsafe { DispatchMessageW(&message) };
            }
        }
    }
    for hook in hooks {
        let _ = unsafe { UnhookWinEvent(hook) };
    }
    if timer != 0 {
        let _ = unsafe { KillTimer(None, timer) };
    }
    let _ = unsafe { DestroyWindow(listener) };
}

fn publish(shared: &Shared, model: &mut Model, desktop: &Win32Desktop, events: &[Event]) {
    if model.update(desktop, events) {
        *shared.snapshot.lock().expect("snapshot lock") = model.snapshot();
        (shared.wake)();
    }
}

// unsafe(ffi)
fn queue(event: Event) {
    PENDING.with_borrow_mut(|pending| pending.push(event));
    if !POSTED.replace(true) {
        let _ = unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_REFRESH, WPARAM(0), LPARAM(0)) };
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
    let whole = event == EVENT_OBJECT_DESTROY
        || event == EVENT_SYSTEM_DESKTOPSWITCH
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
unsafe extern "system" fn on_listener_message(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if matches!(message, WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED) {
        queue(Event::DisplaysChanged);
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// A hidden top-level window, which display changes are broadcast to.
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
