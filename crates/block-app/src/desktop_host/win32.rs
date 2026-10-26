use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use be_win32::{EntryId, EntryKind, FocusEvent, Handle, Hotkey, Program, Snapshot, Watcher};
use be_win32_capture::{Capture, Device, Target};
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Context, Document, Drawing, Forwarded, ForwardedInput, NodeId, PointerPhase, Rect, Setup,
};
use block_plugin_api::{HostImage, HostProgram, HostWindow, HostWindowId};

use super::texture_draw::TextureDraw;
use super::win32_map::{self, SurfaceNotice, entry_id};
use crate::host::HostItem;
use crate::surfaces::HostItemFace;

const HOST_FOREGROUND_REFUSED: &str =
    "Windows did not let Block come back to the front. Click Block's window to use it again.";
const ICON_BUDGET: usize = block_plugin_api::MAX_BLOB_BYTES / 2;
const WAITING_ICON_SIZE: u32 = 64;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
}

struct Captured {
    capture: Capture,
    drawn: u64,
    cursor: bool,
    users: Vec<(u64, Signal<Option<Drawing>>)>,
}

#[derive(Clone, Copy)]
struct Signals {
    snapshot: Signal<Arc<Snapshot>>,
    focused: Signal<Option<EntryId>>,
    refused: Signal<HashMap<EntryId, SurfaceNotice>>,
}

struct Host {
    watcher: Arc<Watcher>,
    secure: bool,
}

thread_local! {
    static SIGNALS: Cell<Option<Signals>> = const { Cell::new(None) };
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static HOST_WINDOW: Cell<Option<Handle>> = const { Cell::new(None) };
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    static DEVICE: RefCell<Option<Device>> = const { RefCell::new(None) };
    static CAPTURES: RefCell<HashMap<EntryId, Captured>> = RefCell::new(HashMap::new());
    static NEXT_USER: Cell<u64> = const { Cell::new(0) };
}

fn signals() -> Option<Signals> {
    SIGNALS.get()
}

fn watcher() -> Option<Arc<Watcher>> {
    HOST.with_borrow(|host| host.as_ref().map(|host| host.watcher.clone()))
}

fn snapshot() -> Arc<Snapshot> {
    signals().map_or_else(Default::default, |signals| signals.snapshot.get_untracked())
}

pub(crate) fn create() {
    SIGNALS.set(Some(Signals {
        snapshot: create_signal(Arc::new(Snapshot::default())),
        focused: create_signal(None),
        refused: create_signal(HashMap::new()),
    }));
}

pub(crate) fn start(setup: &Setup) {
    if let Some(gpu) = setup.get::<beui::GpuSetup>() {
        GPU.set(Some(Gpu {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            srgb: gpu.format.is_srgb(),
        }));
    }
    if let Some(hwnd) = setup
        .get::<beui::NativeWindow>()
        .and_then(beui::NativeWindow::win32)
    {
        HOST_WINDOW.set(Some(Handle(hwnd as u64)));
    }
}

fn spawn() {
    if HOST.with_borrow(Option::is_some) {
        return;
    }
    let watcher = match Watcher::spawn(crate::host::wake) {
        Ok(watcher) => Arc::new(watcher),
        Err(error) => {
            eprintln!("block-app: the Windows windows are not watched: {error}");
            crate::notices::report(format!(
                "Windows' windows and displays cannot be shown, since watching them did not start: {error}"
            ));
            return;
        }
    };
    watcher.set_host_window(HOST_WINDOW.get());
    if let Err(error) = watcher.set_hotkey(Some(Hotkey::DEFAULT)) {
        eprintln!("block-app: the shortcut back to Block was not registered: {error}");
        crate::notices::report(format!(
            "{} cannot bring Block back from a Windows window: {error}",
            Hotkey::DEFAULT
        ));
    }
    HOST.set(Some(Host {
        watcher,
        secure: false,
    }));
}

pub(crate) fn environment() -> Vec<(String, String)> {
    Vec::new()
}

pub(crate) fn running() -> bool {
    HOST.with_borrow(Option::is_some)
}

fn device() -> Option<Device> {
    DEVICE.with_borrow_mut(|held| {
        if held.is_none() {
            let made = GPU.with_borrow(|gpu| {
                let gpu = gpu.as_ref()?;
                Device::new(&gpu.device, &gpu.queue, gpu.srgb)
                    .map_err(|error| {
                        eprintln!("block-app: Windows cannot be captured: {error}");
                        crate::notices::report(format!(
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

pub(crate) fn before(_context: &Context, _rect: Rect, document: &mut Document) {
    let Some(watcher) = watcher() else {
        return;
    };
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
        Some(newly)
    });
    if newly_secure == Some(true) {
        crate::notices::report(win32_map::SECURE_DESKTOP.to_owned());
    }
    for event in &events {
        if *event == FocusEvent::HostForegroundRefused {
            crate::notices::report(HOST_FOREGROUND_REFUSED.to_owned());
        }
    }
    let frames = CAPTURES.with_borrow_mut(|captures| {
        let mut frames = Vec::new();
        captures.retain(|id, captured| {
            if let Some(lost) = captured.capture.lost() {
                eprintln!("block-app: the capture of entry {} ended: {lost:?}", id.0);
                frames.extend(captured.users.iter().map(|(_, drawing)| (*drawing, None)));
                return false;
            }
            let cursor = focused == Some(*id);
            if cursor != captured.cursor {
                captured.cursor = cursor;
                captured.capture.set_cursor(cursor);
            }
            if captured.capture.version() != captured.drawn
                && let Some(frame) = captured.capture.latest()
            {
                captured.drawn = frame.version();
                let texture = frame.texture().clone();
                let drawing = beui::drawing(TextureDraw::new(texture, frame));
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
    beui::reactive::enter(document, || {
        if signals.snapshot.get_untracked().revision != snapshot.revision {
            signals.snapshot.set(snapshot);
        }
        if signals.focused.get_untracked() != focused {
            signals.focused.set(focused);
        }
        let started: Vec<EntryId> = events
            .iter()
            .filter_map(|event| match *event {
                FocusEvent::Started(id) | FocusEvent::Followed { to: id, .. } => Some(id),
                _ => None,
            })
            .collect();
        if !started.is_empty() {
            signals.refused.update(|refused| {
                for id in started {
                    refused.remove(&id);
                }
            });
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
    watcher().map_or(0, |watcher| watcher.snapshot().revision)
}

pub(crate) fn listed(hidden: &[HostWindowId]) -> Vec<HostWindow> {
    watcher().map_or_else(Vec::new, |watcher| {
        win32_map::listed(&watcher.snapshot(), hidden)
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

pub(crate) fn fullscreen(_window: HostWindowId, _fullscreen: bool) {}

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

fn refuse(id: EntryId, notice: Option<SurfaceNotice>) {
    if let Some(signals) = signals() {
        signals.refused.update(|refused| match notice {
            Some(notice) => {
                refused.insert(id, notice);
            }
            None => {
                refused.remove(&id);
            }
        });
    }
}

fn focus_entry(id: EntryId, at: Option<be_win32::Point>) {
    let Some(watcher) = watcher() else {
        return;
    };
    match watcher.focus(id, at) {
        Ok(()) => refuse(id, None),
        Err(error) => {
            eprintln!("block-app: entry {} was not focused: {error}", id.0);
            refuse(id, Some(SurfaceNotice::Focus(error)));
        }
    }
}

pub(crate) fn focus(window: HostWindowId) {
    focus_entry(entry_id(window), None);
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
                crate::notices::report(format!("The program did not start: {error}"));
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

fn acquire(id: EntryId, drawing: Signal<Option<Drawing>>) -> Option<u64> {
    let entry = snapshot().entry(id).cloned()?;
    let user = NEXT_USER.get() + 1;
    NEXT_USER.set(user);
    let held = CAPTURES.with_borrow_mut(|captures| {
        if let Some(captured) = captures.get_mut(&id) {
            captured.users.push((user, drawing));
            captured.drawn = 0;
            crate::host::wake();
            return true;
        }
        false
    });
    if held {
        return Some(user);
    }
    let device = device()?;
    let target = match entry.kind {
        EntryKind::Display => Target::Monitor(entry.handle.0 as isize),
        EntryKind::Window => Target::Window(entry.handle.0 as isize),
    };
    match Capture::start(&device, target, |_| crate::host::wake()) {
        Ok(capture) => {
            CAPTURES.with_borrow_mut(|captures| {
                captures.insert(
                    id,
                    Captured {
                        capture,
                        drawn: 0,
                        cursor: false,
                        users: vec![(user, drawing)],
                    },
                );
            });
            Some(user)
        }
        Err(error) => {
            eprintln!("block-app: {} cannot be captured: {error}", entry.title);
            None
        }
    }
}

fn release(id: EntryId, user: u64) {
    let capture = CAPTURES.with_borrow_mut(|captures| {
        let captured = captures.get_mut(&id)?;
        captured.users.retain(|(held, _)| *held != user);
        match captured.users.is_empty() {
            true => captures.remove(&id),
            false => None,
        }
    });
    drop(capture);
}

fn notice_item(notice: &SurfaceNotice) -> HostItem {
    HostItem::Notice {
        text: notice.text(),
        busy: false,
        icon: Some(match notice {
            SurfaceNotice::Elevated => beui::icons::ICON_SHIELD_USER,
            SurfaceNotice::SecureDesktop => beui::icons::ICON_SHIELD_ALERT,
            SurfaceNotice::Focus(_) => beui::icons::ICON_TRIANGLE_ALERT,
        }),
    }
}

fn forwarded(id: EntryId, pressed: &Cell<bool>, input: ForwardedInput) {
    let size = input.rect.size();
    for event in &input.events {
        let Forwarded::Pointer { event, over } = event else {
            continue;
        };
        let local = over.map_or(event.pos - input.rect.min.to_vec2(), |over| over.local);
        if !event.pointer.is_controller() {
            if let PointerPhase::Down(_) = event.phase {
                let snapshot = snapshot();
                let at = snapshot
                    .entry(id)
                    .map(|entry| win32_map::frame_point(entry.frame, size, local));
                focus_entry(id, at);
            }
            continue;
        }
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
                refuse(id, Some(SurfaceNotice::Elevated));
            }
            Err(error) => eprintln!("block-app: input did not reach {}: {error}", entry.title),
        }
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
    if let Some(user) = acquire(id, drawing) {
        on_cleanup(move || release(id, user));
    }
    let snapshot = signals.snapshot;
    let refused = signals.refused;
    let notice = create_memo(move || {
        let snapshot = snapshot.get();
        let entry = snapshot.entry(id)?;
        refused.with(|refused| win32_map::surface_notice(&snapshot, entry, refused.get(&id)))
    });
    let focused_signal = signals.focused;
    let focused = create_memo(move || focused_signal.get() == Some(id));
    let dim = create_memo(move || match notice.get().is_some() {
        true => ColorRole::Dim,
        false => ColorRole::None,
    });
    let pressed = Rc::new(Cell::new(false));
    view! {
        <Interactive
            focusable=true
            @test_id={format!("win32.window.{}", id.0)}
            on_forward={move |input: ForwardedInput| forwarded(id, &pressed, input)}
        >
            <Layers>
                <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
                <Frame fill={dim} />
                <Dynamic value={notice}>|notice: Option<SurfaceNotice>| {
                    match notice {
                        Some(notice) => {
                            let content = create_memo(move || notice_item(&notice));
                            view! {
                                <Frame align=beui::Align::Center>
                                    <HostItemFace content />
                                </Frame>
                            }
                        }
                        None => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
                <Dynamic value={focused}>|focused: bool| {
                    match focused {
                        true => {
                            let content = create_memo(|| HostItem::Notice {
                                text: format!(
                                    "Windows has the mouse and keyboard. Press {} to come back to Block.",
                                    Hotkey::DEFAULT
                                ),
                                busy: false,
                                icon: Some(beui::icons::ICON_KEYBOARD),
                            });
                            view! {
                                <List spacing=Space::Sm align=Align::Center>
                                    <HostItemFace content />
                                </List>
                            }
                        }
                        false => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
            </Layers>
        </Interactive>
    }
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
        let (sender, receiver) = crate::host::waking_channel();
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
            crate::notices::report(format!("{key} could not be opened: it is not installed"));
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
        crate::plugin_host::publish::<block_plugin_api::PowerWatch>(&available);
        available
    });
    if let Some(action) = crate::plugin_host::take_calls(block_plugin_api::requested::power).pop()
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
                crate::notices::report(format!("Windows refused to {action:?}: {error}"));
            }
        });
    if let Err(error) = spawned {
        eprintln!("block-app: the power request was not made: {error}");
    }
}
