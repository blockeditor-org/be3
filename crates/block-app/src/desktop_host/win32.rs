use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use be_win32::{EntryId, EntryKind, FocusEvent, Handle, Hotkey, Program, Snapshot, Watcher};
use be_win32_capture::{Capture, Device, Lost, Target};
use beui::native::{Document, draw_gpu};
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Forwarded, ForwardedInput, NodeId, PointerPhase, Pos2, Rect, Vec2, WindowPlacement,
    WindowReport, WindowRole, native::Context, native::Drawing, native::Setup,
};
use block_plugin_api::{HostImage, HostProgram, HostWindow, HostWindowId};

use super::texture_draw::TextureDraw;
use super::win32_map::{self, ContentPress, HotkeyState, Notes, Recovery, Refusal, entry_id};
use super::win32_overlay::EntryOverlay;
use super::win32_shape::Shaping;

const HOST_FOREGROUND_REFUSED: &str =
    "Windows did not let Block come back to the front. Click Block's window to use it again.";
const ICON_BUDGET: usize = block_plugin_api::MAX_BLOB_BYTES / 2;
const WAITING_ICON_SIZE: u32 = 64;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Source {
    Entry(EntryId),
    Popup { entry: EntryId, handle: Handle },
}

impl Source {
    fn entry(self) -> EntryId {
        match self {
            Self::Entry(entry) | Self::Popup { entry, .. } => entry,
        }
    }
}

struct Captured {
    capture: Capture,
    target: Target,
    title: String,
    restarted: bool,
    drawn: u64,
    cursor: bool,
    users: Vec<(u64, Signal<Option<Drawing>>)>,
}

#[derive(Clone, Copy)]
struct Signals {
    snapshot: Signal<Arc<Snapshot>>,
}

struct Host {
    watcher: Arc<Watcher>,
    secure: bool,
    listed: bool,
}

thread_local! {
    static SIGNALS: Cell<Option<Signals>> = const { Cell::new(None) };
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static HOST_WINDOW: Cell<Option<Handle>> = const { Cell::new(None) };
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static DEVICE: RefCell<Option<Device>> = const { RefCell::new(None) };
    static CAPTURES: RefCell<HashMap<Source, Captured>> = RefCell::new(HashMap::new());
    static NEXT_USER: Cell<u64> = const { Cell::new(0) };
    static SHAPING: RefCell<Shaping> = RefCell::new(Shaping::default());
    static CHANGES: Cell<u64> = const { Cell::new(0) };
    static NOTES: RefCell<Notes> = RefCell::new(Notes::default());
}

fn edit_notes(change: impl FnOnce(&mut Notes)) {
    let edited = NOTES.with_borrow_mut(|notes| {
        let before = notes.clone();
        change(notes);
        *notes != before
    });
    if edited {
        changed();
    }
}

fn signals() -> Option<Signals> {
    SIGNALS.get()
}

fn watcher() -> Option<Arc<Watcher>> {
    HOST.with_borrow(|host| host.as_ref().map(|host| host.watcher.clone()))
}

fn changed() {
    CHANGES.set(CHANGES.get() + 1);
    block_app_base::host::wake();
}

fn snapshot() -> Arc<Snapshot> {
    signals().map_or_else(Default::default, |signals| signals.snapshot.get_untracked())
}

pub(crate) fn create() {
    SIGNALS.set(Some(Signals {
        snapshot: create_signal(Arc::new(Snapshot::default())),
    }));
}

pub(crate) fn start(setup: &Setup) {
    if let Some(gpu) = setup.get::<beui_host::native::GpuSetup>() {
        GPU.set(Some(Gpu {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            srgb: gpu.format.is_srgb(),
        }));
    }
    if let Some(hwnd) = setup
        .get::<beui::native::NativeWindow>()
        .and_then(beui::native::NativeWindow::win32)
    {
        HOST_WINDOW.set(Some(Handle(hwnd as u64)));
    }
}

fn spawn() {
    if HOST.with_borrow(Option::is_some) {
        return;
    }
    let watcher = match Watcher::spawn(block_app_base::host::wake) {
        Ok(watcher) => Arc::new(watcher),
        Err(error) => {
            eprintln!("block-app: the Windows windows are not watched: {error}");
            block_app_base::notices::report(format!(
                "Windows' windows and displays cannot be shown, since watching them did not start: {error}"
            ));
            return;
        }
    };
    eprintln!("block-app: watching Windows' windows and displays");
    watcher.set_host_window(HOST_WINDOW.get());
    if HOST_WINDOW.get().is_none() {
        eprintln!("block-app: Block's own window is unknown, so focus cannot bring it back");
    }
    if let Err(error) = watcher.set_hotkey(Some(Hotkey::DEFAULT)) {
        hotkey_refused(&error);
    }
    HOST.set(Some(Host {
        watcher,
        secure: false,
        listed: false,
    }));
}

pub(crate) fn environment() -> Vec<(String, String)> {
    Vec::new()
}

pub(crate) fn running() -> bool {
    HOST.with_borrow(Option::is_some)
}

fn hotkey_refused(error: &be_win32::HotkeyError) {
    eprintln!(
        "block-app: the shortcut back to Block, {}, was not registered: {error}",
        Hotkey::DEFAULT
    );
    block_app_base::notices::report(format!(
        "{} cannot bring Block back from a Windows window: {error}",
        Hotkey::DEFAULT
    ));
}

fn log_event(snapshot: &Snapshot, event: &FocusEvent) {
    let name = |id: EntryId| {
        snapshot.entry(id).map_or_else(
            || format!("entry {}", id.0),
            |entry| format!("\"{}\"", entry.title),
        )
    };
    match event {
        FocusEvent::Started(id) => eprintln!("block-app: focus mode started on {}", name(*id)),
        FocusEvent::Followed { from, to } => {
            eprintln!(
                "block-app: focus mode moved from {} to {}",
                name(*from),
                name(*to)
            );
        }
        FocusEvent::Ended { entry, reason } => {
            eprintln!(
                "block-app: focus mode on {} ended: {reason:?}",
                name(*entry)
            );
        }
        FocusEvent::Refused { entry, error } => {
            eprintln!(
                "block-app: focus mode on {} did not start: {error}",
                name(*entry)
            );
        }
        FocusEvent::Suspended(id) => {
            eprintln!(
                "block-app: focus mode on {} paused for the secure desktop",
                name(*id)
            );
        }
        FocusEvent::Resumed(id) => eprintln!("block-app: focus mode on {} resumed", name(*id)),
        FocusEvent::Hotkey => eprintln!("block-app: {} pressed", Hotkey::DEFAULT),
        FocusEvent::HostForegroundRefused => {
            eprintln!("block-app: Windows did not let Block come back to the front");
        }
        FocusEvent::HotkeySet(Some(hotkey)) => {
            eprintln!("block-app: {hotkey} brings Block back from a Windows window");
        }
        FocusEvent::HotkeySet(None) => {}
        FocusEvent::HotkeyRefused(error) => hotkey_refused(error),
    }
}

fn describe(target: Target, title: &str) -> String {
    match target {
        Target::Monitor(_) => format!("display \"{title}\""),
        Target::Window(_) => format!("window \"{title}\""),
    }
}

fn device() -> Option<Device> {
    DEVICE.with_borrow_mut(|held| {
        if held.is_none() {
            let made = GPU.with_borrow(|gpu| {
                let gpu = gpu.as_ref()?;
                Device::new(&gpu.device, &gpu.queue, gpu.srgb)
                    .inspect(|device| {
                        eprintln!("block-app: capturing Windows on {}", device.adapter());
                    })
                    .map_err(|error| {
                        eprintln!("block-app: Windows cannot be captured: {error}");
                        block_app_base::notices::report(format!(
                            "Windows' windows and displays cannot be drawn: {error}"
                        ));
                    })
                    .ok()
            });
            *held = made;
        }
        held.clone()
    })
}

fn start_capture(target: Target) -> Result<Capture, String> {
    let device = device().ok_or_else(|| "there is no GPU to draw it with".to_owned())?;
    Capture::start(&device, target, |_| block_app_base::host::wake())
}

pub(crate) fn before(_context: &Context, rect: Rect, document: &mut Document) {
    let Some(watcher) = watcher() else {
        return;
    };
    if SHAPING.with_borrow_mut(|shaping| shaping.set_area(rect)) {
        changed();
    }
    let Some(signals) = signals() else {
        return;
    };
    let snapshot = watcher.snapshot();
    let events = watcher.take_focus_events();
    let focused = watcher.focused().map(|focused| focused.entry);
    let secure = snapshot.secure_desktop();
    let newly_secure = HOST.with_borrow_mut(|host| {
        let host = host.as_mut()?;
        let newly = secure && !host.secure;
        host.secure = secure;
        if !host.listed && snapshot.revision > 0 {
            host.listed = true;
            let displays = snapshot
                .entries
                .iter()
                .filter(|entry| entry.kind == EntryKind::Display)
                .count();
            eprintln!(
                "block-app: Windows has {displays} displays and {} windows",
                snapshot.entries.len() - displays
            );
        }
        Some(newly)
    });
    if newly_secure == Some(true) {
        eprintln!("block-app: Windows switched to the secure desktop");
        block_app_base::notices::report(win32_map::SECURE_DESKTOP.to_owned());
    }
    for event in &events {
        log_event(&snapshot, event);
        if *event == FocusEvent::HostForegroundRefused {
            block_app_base::notices::report(HOST_FOREGROUND_REFUSED.to_owned());
        }
    }
    let frames = CAPTURES.with_borrow_mut(|captures| {
        let mut frames = Vec::new();
        captures.retain(|source, captured| {
            if let Some(lost) = captured.capture.lost() {
                eprintln!(
                    "block-app: the capture of {} ended: {lost:?}",
                    describe(captured.target, &captured.title)
                );
                let failed = match &lost {
                    Lost::Closed => None,
                    Lost::Failed(error) => Some(error.as_str()),
                };
                let notice = match win32_map::recovery(failed, captured.restarted, &captured.title)
                {
                    Recovery::Stop => None,
                    Recovery::Notice(notice) => Some(notice),
                    Recovery::Restart => match start_capture(captured.target) {
                        Ok(capture) => {
                            eprintln!(
                                "block-app: the capture of {} started again",
                                describe(captured.target, &captured.title)
                            );
                            captured.capture = capture;
                            captured.restarted = true;
                            captured.drawn = 0;
                            captured.cursor = false;
                            return true;
                        }
                        Err(error) => {
                            eprintln!(
                                "block-app: the capture of {} did not start again: {error}",
                                describe(captured.target, &captured.title)
                            );
                            win32_map::recovery(Some(&error), true, &captured.title).notice()
                        }
                    },
                };
                if let Some(notice) = notice {
                    block_app_base::notices::report(notice);
                }
                frames.extend(captured.users.iter().map(|(_, drawing)| (*drawing, None)));
                return false;
            }
            let cursor = focused == Some(source.entry());
            if cursor != captured.cursor {
                captured.cursor = cursor;
                captured.capture.set_cursor(cursor);
            }
            if captured.capture.version() != captured.drawn
                && let Some(frame) = captured.capture.latest()
            {
                captured.drawn = frame.version();
                let texture = frame.texture().clone();
                let drawing = beui::native::drawing(TextureDraw::new(texture, frame));
                frames.extend(
                    captured
                        .users
                        .iter()
                        .map(|(_, signal)| (*signal, Some(drawing.clone()))),
                );
            }
            true
        });
        frames
    });
    let started: Vec<EntryId> = events
        .iter()
        .filter_map(|event| match *event {
            FocusEvent::Started(id) | FocusEvent::Followed { to: id, .. } => Some(id),
            _ => None,
        })
        .collect();
    edit_notes(|notes| {
        notes.focused = focused;
        for id in started {
            notes.refused.remove(&id);
        }
        for event in &events {
            match event {
                FocusEvent::HotkeySet(Some(hotkey)) => {
                    notes.hotkey = HotkeyState::Registered(*hotkey);
                }
                FocusEvent::HotkeySet(None) => notes.hotkey = HotkeyState::Waiting,
                FocusEvent::HotkeyRefused(error) => {
                    notes.hotkey = HotkeyState::Refused(error.clone());
                }
                FocusEvent::Refused { entry, error } => {
                    if let Some(refusal) = win32_map::focus_refusal(error) {
                        notes.refused.insert(*entry, refusal);
                    }
                }
                FocusEvent::Ended { entry, reason } => {
                    if let Some(refusal) = win32_map::ended_refusal(*reason) {
                        notes.refused.insert(*entry, refusal);
                    }
                }
                _ => {}
            }
        }
        let live = |id: &EntryId| snapshot.entry(*id).is_some();
        notes.refused.retain(|id, _| live(id));
        notes.hovered = notes.hovered.filter(live);
    });
    beui::native::enter(document, || {
        if signals.snapshot.get_untracked().revision != snapshot.revision {
            signals.snapshot.set(snapshot);
        }
        for (signal, drawing) in frames {
            signal.set(drawing);
        }
    });
}

pub(crate) fn after(_context: &Context, _document: &mut Document) {}

pub(crate) fn replace_gpu(setup: &Setup) {
    CAPTURES.with_borrow_mut(HashMap::clear);
    DEVICE.set(None);
    start(setup);
}

pub(crate) fn exiting() {
    CAPTURES.with_borrow_mut(HashMap::clear);
    DEVICE.set(None);
    let host = HOST.with_borrow_mut(Option::take);
    if let Some(host) = &host {
        host.watcher.release();
    }
    drop(host);
}

pub(crate) fn set_owned(owned: bool) {
    if owned {
        spawn();
    }
}

pub(crate) fn revision() -> u64 {
    watcher().map_or(0, |watcher| watcher.snapshot().revision) + CHANGES.get()
}

pub(crate) fn listed(hidden: &[HostWindowId]) -> Vec<HostWindow> {
    watcher().map_or_else(Vec::new, |watcher| {
        let snapshot = watcher.snapshot();
        SHAPING.with_borrow_mut(|shaping| {
            shaping.observe(&snapshot);
            NOTES.with_borrow(|notes| win32_map::listed(&snapshot, hidden, shaping, notes))
        })
    })
}

pub(crate) fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
    Vec::new()
}

pub(crate) fn close(window: HostWindowId) {
    let snapshot = snapshot();
    let Some(entry) = snapshot.entry(entry_id(window)) else {
        return;
    };
    if entry.kind == EntryKind::Window
        && let Err(error) = be_win32::close_window(entry.handle)
    {
        eprintln!("block-app: {} did not close: {error}", entry.title);
    }
}

pub(crate) fn fullscreen(window: HostWindowId, fullscreen: bool) {
    let Some(watcher) = watcher() else {
        return;
    };
    let snapshot = watcher.snapshot();
    let id = entry_id(window);
    let handle = SHAPING.with_borrow_mut(|shaping| match fullscreen {
        true => shaping.enter_fullscreen(&snapshot, id),
        false => shaping.leave_fullscreen(&snapshot, id),
    });
    if let Some(handle) = handle {
        be_win32::set_maximized(handle, fullscreen);
    }
    changed();
}

fn resize(id: EntryId, size: Vec2) {
    let snapshot = snapshot();
    let asked = SHAPING.with_borrow_mut(|shaping| shaping.placed(&snapshot, id, size));
    let (Some((width, height)), Some(entry)) = (asked, snapshot.entry(id)) else {
        return;
    };
    if let Err(error) = be_win32::resize_window(entry.handle, width, height) {
        eprintln!("block-app: {} was not resized: {error}", entry.title);
    }
}

pub(crate) fn show(window: HostWindowId) {
    if let Some(watcher) = watcher() {
        watcher.show(entry_id(window));
    }
}

pub(crate) fn hide(window: HostWindowId) {
    if let Some(watcher) = watcher() {
        watcher.hide(entry_id(window));
    }
}

fn refuse(id: EntryId, refusal: Option<Refusal>) {
    edit_notes(|notes| match refusal {
        Some(refusal) => {
            notes.refused.insert(id, refusal);
        }
        None => {
            notes.refused.remove(&id);
        }
    });
}

fn focus_entry(id: EntryId, at: Option<be_win32::Point>) {
    let Some(watcher) = watcher() else {
        return;
    };
    match watcher.focus(id, at) {
        Ok(()) => refuse(id, None),
        Err(error) => {
            eprintln!("block-app: entry {} was not focused: {error}", id.0);
            refuse(id, win32_map::focus_refusal(&error));
        }
    }
}

pub(crate) fn focus(window: HostWindowId) {
    if let Some(id) = win32_map::shell_focus(window) {
        focus_entry(id, None);
    }
}

fn spawn_launch(work: impl FnOnce() -> std::io::Result<be_win32::Launch> + Send + 'static) {
    let Some(watcher) = watcher() else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("launch".to_owned())
        .spawn(move || match work() {
            Ok(launch) => watcher.launched(launch),
            Err(error) => {
                eprintln!("block-app: the program did not start: {error}");
                block_app_base::notices::report(format!("The program did not start: {error}"));
            }
        });
    if let Err(error) = spawned {
        eprintln!("block-app: the program was not started: {error}");
    }
}

pub(crate) fn launch(command: String) -> bool {
    exec(be_win32::command_line::split_arguments(&command), None)
}

pub(crate) fn exec(arguments: Vec<String>, launch: Option<u64>) -> bool {
    if !running() || arguments.is_empty() {
        return false;
    }
    spawn_launch(move || be_win32::exec(&arguments, launch));
    true
}

#[component]
pub(crate) fn WindowActions() -> NodeId {
    view! {
        <Frame />
    }
}

fn acquire(source: Source, drawing: Signal<Option<Drawing>>) -> Option<u64> {
    let snapshot = snapshot();
    let entry = snapshot.entry(source.entry())?;
    let user = NEXT_USER.get() + 1;
    NEXT_USER.set(user);
    let held = CAPTURES.with_borrow_mut(|captures| {
        if let Some(captured) = captures.get_mut(&source) {
            captured.users.push((user, drawing));
            captured.drawn = 0;
            block_app_base::host::wake();
            return true;
        }
        false
    });
    if held {
        return Some(user);
    }
    let target = match (source, entry.kind) {
        (Source::Popup { handle, .. }, _) => Target::Window(handle.0 as isize),
        (Source::Entry(_), EntryKind::Display) => Target::Monitor(entry.handle.0 as isize),
        (Source::Entry(_), EntryKind::Window) => Target::Window(entry.handle.0 as isize),
    };
    match start_capture(target) {
        Ok(capture) => {
            eprintln!("block-app: capturing {}", describe(target, &entry.title));
            CAPTURES.with_borrow_mut(|captures| {
                captures.insert(
                    source,
                    Captured {
                        capture,
                        target,
                        title: entry.title.clone(),
                        restarted: false,
                        drawn: 0,
                        cursor: false,
                        users: vec![(user, drawing)],
                    },
                );
            });
            Some(user)
        }
        Err(error) => {
            eprintln!(
                "block-app: {} cannot be captured: {error}",
                describe(target, &entry.title)
            );
            if DEVICE.with_borrow(Option::is_some) && matches!(source, Source::Entry(_)) {
                block_app_base::notices::report(format!(
                    "{} cannot be drawn: {error}",
                    match entry.title.is_empty() {
                        true => "A window",
                        false => &entry.title,
                    }
                ));
            }
            None
        }
    }
}

fn release(source: Source, user: u64) {
    let capture = CAPTURES.with_borrow_mut(|captures| {
        let captured = captures.get_mut(&source)?;
        captured.users.retain(|(held, _)| *held != user);
        match captured.users.is_empty() {
            true => captures.remove(&source),
            false => None,
        }
    });
    if let Some(captured) = &capture {
        eprintln!(
            "block-app: stopped capturing {}",
            describe(captured.target, &captured.title)
        );
    }
    drop(capture);
}

fn clicked(press: &RefCell<ContentPress>, input: &ForwardedInput) -> Vec<Pos2> {
    let size = input.rect.size();
    let mut clicks = Vec::new();
    let mut press = press.borrow_mut();
    for event in &input.events {
        let Forwarded::Pointer { event, over } = event else {
            continue;
        };
        if event.pointer.is_controller() {
            continue;
        }
        let local = over.map_or(event.pos - input.rect.min.to_vec2(), |over| over.local);
        match event.phase {
            PointerPhase::Down(_) => press.down(event.pointer, local),
            PointerPhase::Move => press.moved(event.pointer, local),
            PointerPhase::Up(_) => clicks.extend(press.up(event.pointer, local, size)),
            PointerPhase::Cancel | PointerPhase::Gone => press.cancel(),
            _ => {}
        }
    }
    clicks
}

fn forwarded(
    id: EntryId,
    pressed: &Cell<bool>,
    press: &RefCell<ContentPress>,
    input: ForwardedInput,
) {
    let size = input.rect.size();
    for local in clicked(press, &input) {
        let snapshot = snapshot();
        let at = snapshot
            .entry(id)
            .map(|entry| win32_map::frame_point(entry.frame, size, local));
        focus_entry(id, at);
    }
    for event in &input.events {
        let Forwarded::Pointer { event, over } = event else {
            continue;
        };
        if !event.pointer.is_controller() {
            continue;
        }
        let local = over.map_or(event.pos - input.rect.min.to_vec2(), |over| over.local);
        let action = match event.phase {
            PointerPhase::Down(button) => {
                pressed.set(true);
                be_win32_input::PointerAction::Press(button)
            }
            PointerPhase::Up(button) => {
                pressed.set(false);
                be_win32_input::PointerAction::Release(button)
            }
            PointerPhase::Move if pressed.get() => be_win32_input::PointerAction::Move,
            PointerPhase::Cancel | PointerPhase::Gone => {
                pressed.set(false);
                continue;
            }
            _ => continue,
        };
        let snapshot = snapshot();
        let Some(entry) = snapshot.entry(id) else {
            continue;
        };
        let injected = be_win32_input::inject(
            &snapshot,
            entry,
            size,
            &be_win32_input::Event::Pointer { at: local, action },
        );
        match injected {
            Ok(()) => refuse(id, None),
            Err(be_win32_input::InjectError::Elevated) => {
                refuse(id, Some(Refusal::Elevated));
            }
            Err(error) => eprintln!("block-app: input did not reach {}: {error}", entry.title),
        }
    }
}

fn popup_forwarded(
    entry: EntryId,
    handle: Handle,
    press: &RefCell<ContentPress>,
    input: ForwardedInput,
) {
    let size = input.rect.size();
    for local in clicked(press, &input) {
        let snapshot = snapshot();
        let at = snapshot.entry(entry).and_then(|toplevel| {
            let popup = toplevel
                .popups
                .iter()
                .find(|popup| popup.handle == handle)?;
            Some(win32_map::popup_point(
                toplevel.frame,
                popup.frame,
                size,
                local,
            ))
        });
        focus_entry(entry, at);
    }
}

#[component]
fn PopupSurface(entry: EntryId, handle: Handle) -> NodeId {
    let source = Source::Popup { entry, handle };
    let drawing = create_signal(None);
    if let Some(user) = acquire(source, drawing) {
        on_cleanup(move || release(source, user));
    }
    let press = RefCell::new(ContentPress::default());
    view! {
        <Interactive
            focusable=true
            press_focus=false
            tab_stop=false
            @test_id={format!("win32.popup.{}", handle.0)}
            on_forward={move |input: ForwardedInput| popup_forwarded(entry, handle, &press, input)}
        >
            <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
        </Interactive>
    }
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    let id = entry_id(window);
    let Some(signals) = signals() else {
        return view! {
            <Frame />
        };
    };
    let drawing = create_signal(None);
    if let Some(user) = acquire(Source::Entry(id), drawing) {
        on_cleanup(move || release(Source::Entry(id), user));
    }
    let snapshot = signals.snapshot;
    let secure = create_memo(move || {
        snapshot.with(|snapshot| {
            snapshot
                .entry(id)
                .is_some_and(|entry| win32_map::secure_notice(snapshot, entry))
        })
    });
    let hovered = create_signal(false);
    create_effect(move || {
        let over = hovered.get();
        untrack(|| {
            edit_notes(|notes| match over {
                true => notes.hovered = Some(id),
                false => notes.hovered = notes.hovered.filter(|held| *held != id),
            });
        });
    });
    on_cleanup(move || {
        edit_notes(|notes| notes.hovered = notes.hovered.filter(|held| *held != id))
    });
    let off_screen = create_memo(move || {
        snapshot.with(|snapshot| {
            snapshot
                .entry(id)
                .map(|entry| win32_map::off_screen_parts(snapshot, entry))
                .unwrap_or_default()
        })
    });
    let pressed = Rc::new(Cell::new(false));
    let press = Rc::new(RefCell::new(ContentPress::default()));
    let rect_held = create_signal(Rect::ZERO);
    let rect = rect_held.read_only();
    create_effect(move || resize(id, rect.get().size()));
    let popups = create_memo(move || {
        let panel = rect.get();
        snapshot.with(|snapshot| {
            snapshot
                .entry(id)
                .map(|entry| {
                    entry
                        .popups
                        .iter()
                        .map(|popup| {
                            let at = win32_map::popup_rect(entry.frame, popup.frame, panel.size());
                            (popup.handle, at.translate(panel.min.to_vec2()))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let hosted = opens_windows();
    let keys = create_memo(move || {
        let hosted = hosted.get();
        popups.with(|popups| {
            popups
                .iter()
                .map(|(handle, at)| {
                    (
                        handle.0,
                        hosted.then(|| (at.min.x.to_bits(), at.min.y.to_bits())),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let parent = use_window().map_or(beui::WindowId::MAIN, |window| window.id);
    let built = view! {
        <Layers>
            <Interactive
                focusable=true
                @test_id={format!("win32.window.{}", id.0)}
                on_hover_change={hovered}
                on_forward={move |input: ForwardedInput| forwarded(id, &pressed, &press, input)}
            >
                <Layers>
                    <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
                    <EntryOverlay secure off_screen />
                </Layers>
            </Interactive>
            <ForEach keys={keys}>|key: (u64, Option<(u32, u32)>)| {
                let handle = Handle(key.0);
                let placed = create_memo(move || {
                    popups.with(|popups| {
                        popups
                            .iter()
                            .find(|(popup, _)| *popup == handle)
                            .map(|(_, at)| *at)
                    })
                });
                let at = create_memo(move || placed.get().map_or(Pos2::ZERO, |at| at.min));
                let size = create_memo(move || placed.get().map_or(Vec2::ZERO, |at| at.size()));
                let open = create_memo(move || placed.get().is_some());
                let placement = WindowPlacement::Over {
                    window: parent,
                    at: at.get_untracked(),
                };
                view! {
                    <Window
                        id={None}
                        size
                        placement
                        at
                        open
                        role={WindowRole::Popup}
                        on_report={|_: WindowReport| {}}
                    >|_: WindowHandle|
                        <PopupSurface entry={id} handle />
                    </Window>
                }
            }</ForEach>
        </Layers>
    };
    let rect_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = rect_measured.get() {
            rect_held.set(measured.rect);
        }
    });
    built
}

enum Scanned {
    Programs(Vec<Program>),
    Icons(Vec<(String, HostImage)>),
}

pub(crate) struct Opener {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) extensions: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Programs {
    programs: Vec<Program>,
    icons: HashMap<String, HostImage>,
    scanning: Option<Receiver<Scanned>>,
    rescan: Option<u32>,
    waiting: Vec<(String, Option<u64>)>,
    scanned: bool,
}

impl Programs {
    pub(crate) fn scanned(&self) -> bool {
        self.scanned
    }

    pub(crate) fn openers(&self) -> Vec<Opener> {
        Vec::new()
    }

    pub(crate) fn open_file(
        &mut self,
        _key: &str,
        _file: &std::path::Path,
        _block: uuid::Uuid,
    ) -> Result<(), String> {
        Err("Files cannot be opened in Windows programs yet.".to_owned())
    }

    pub(crate) fn scan(&mut self, pixels: u32) {
        if self.scanning.is_some() {
            self.rescan = Some(pixels);
            return;
        }
        let (sender, receiver) = block_app_base::host::waking_channel();
        self.scanning = Some(receiver);
        let spawned = std::thread::Builder::new()
            .name("programs".to_owned())
            .spawn(move || {
                let programs = be_win32::start_menu_programs();
                if sender.send(Scanned::Programs(programs.clone())).is_err() {
                    return;
                }
                let icons = programs
                    .iter()
                    .filter_map(|program| {
                        let icon = be_win32::program_icon(program, pixels)?;
                        Some((
                            program.id.clone(),
                            HostImage {
                                width: icon.width,
                                height: icon.height,
                                rgba: icon.rgba,
                            },
                        ))
                    })
                    .collect();
                let _ = sender.send(Scanned::Icons(icons));
            });
        if let Err(error) = spawned {
            eprintln!("block-app: the programs were not listed: {error}");
            self.scanning = None;
        }
    }

    pub(crate) fn receive(&mut self) -> bool {
        let Some(receiver) = &self.scanning else {
            return false;
        };
        let mut changed = false;
        loop {
            match receiver.try_recv() {
                Ok(Scanned::Programs(programs)) => {
                    self.programs = programs;
                    self.scanned = true;
                    changed = true;
                }
                Ok(Scanned::Icons(icons)) => {
                    self.icons = icons.into_iter().collect();
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.scanning = None;
                    if let Some(pixels) = self.rescan.take() {
                        self.scan(pixels);
                    }
                    break;
                }
            }
        }
        if self.scanned {
            for (key, launch) in std::mem::take(&mut self.waiting) {
                self.launch(&key, launch);
            }
        }
        changed
    }

    pub(crate) fn listed(&self) -> Vec<HostProgram> {
        let mut room = ICON_BUDGET;
        self.programs
            .iter()
            .map(|program| {
                let icon = self
                    .icons
                    .get(&program.id)
                    .filter(|icon| {
                        let fits = icon.rgba.len() <= room;
                        if fits {
                            room -= icon.rgba.len();
                        }
                        fits
                    })
                    .cloned();
                win32_map::host_program(program, icon)
            })
            .collect()
    }

    pub(crate) fn launch(&mut self, key: &str, launch: Option<u64>) -> bool {
        if !self.scanned {
            self.waiting.push((key.to_owned(), launch));
            self.scan(WAITING_ICON_SIZE);
            return false;
        }
        let Some(program) = self
            .programs
            .iter()
            .find(|program| program.id == key)
            .cloned()
        else {
            eprintln!("block-app: no program {key} is installed");
            block_app_base::notices::report(format!("{key} could not be opened: it is not installed"));
            return false;
        };
        if !running() {
            return false;
        }
        spawn_launch(move || be_win32::launch(&program, launch));
        true
    }
}

thread_local! {
    static POWER: Cell<Option<block_plugin_api::PowerAvailability>> = const { Cell::new(None) };
}

pub(crate) fn frame() {
    if !running() {
        return;
    }
    let available = POWER.get().unwrap_or_else(|| {
        let available = win32_map::power_availability(be_win32::power_availability());
        POWER.set(Some(available));
        block_app_base::watches::publish::<block_plugin_api::PowerWatch>(&available);
        available
    });
    if let Some(action) = block_app_base::watches::take_calls(block_plugin_api::requested::power).pop()
        && available.allows(action)
    {
        request_power(action);
    }
}

fn request_power(action: block_plugin_api::PowerAction) {
    let action = win32_map::power_action(action);
    let spawned = std::thread::Builder::new()
        .name("power".to_owned())
        .spawn(move || {
            if let Err(error) = be_win32::request_power(action) {
                eprintln!("block-app: Windows did not {action:?}: {error}");
                block_app_base::notices::report(format!("Windows refused to {action:?}: {error}"));
            }
        });
    if let Err(error) = spawned {
        eprintln!("block-app: the power request was not made: {error}");
    }
}
