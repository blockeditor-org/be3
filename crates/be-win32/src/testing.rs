use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    Desktop, Handle, Image, InputDesktop, IntegrityLevel, Monitor, Process, RawWindow, Rect,
};

pub const OWN_PROCESS: u32 = 1;

thread_local! {
    static TITLES: RefCell<HashMap<Handle, String>> = RefCell::new(HashMap::new());
}

pub fn title_of(handle: Handle) -> String {
    TITLES.with_borrow(|titles| titles.get(&handle).cloned().unwrap_or_default())
}

pub fn rename(handle: Handle, title: &str) {
    TITLES.with_borrow_mut(|titles| titles.insert(handle, title.to_owned()));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    Title(Handle),
    Icon(Handle),
    AppId(Handle),
}

#[derive(Default)]
pub struct FakeDesktop {
    pub windows: RefCell<Vec<RawWindow>>,
    pub processes: RefCell<HashMap<u32, Process>>,
    pub icons: RefCell<HashMap<Handle, Arc<Image>>>,
    pub app_ids: RefCell<HashMap<Handle, String>>,
    pub monitors: RefCell<Vec<Monitor>>,
    pub foreground: Cell<Option<Handle>>,
    pub input_desktop: RefCell<InputDesktop>,
    pub icon_reads: Cell<usize>,
    pub process_reads: Cell<usize>,
    pub reads: RefCell<Vec<Read>>,
}

impl FakeDesktop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, window: RawWindow) {
        self.processes
            .borrow_mut()
            .entry(window.process_id)
            .or_insert_with(|| Process {
                exe: Some(PathBuf::from(format!(
                    "C:\\Program Files\\App{}\\app.exe",
                    window.process_id
                ))),
                integrity: Some(IntegrityLevel::MEDIUM),
            });
        self.windows.borrow_mut().push(window);
    }

    pub fn change(&self, handle: Handle, change: impl FnOnce(&mut RawWindow)) {
        let mut windows = self.windows.borrow_mut();
        change(
            windows
                .iter_mut()
                .find(|window| window.handle == handle)
                .expect("the window exists"),
        );
    }

    pub fn remove(&self, handle: Handle) {
        self.windows
            .borrow_mut()
            .retain(|window| window.handle != handle);
    }
}

impl Desktop for FakeDesktop {
    fn windows(&self) -> Vec<RawWindow> {
        self.windows.borrow().clone()
    }

    fn title(&self, window: Handle) -> String {
        self.reads.borrow_mut().push(Read::Title(window));
        title_of(window)
    }

    fn process(&self, process_id: u32) -> Process {
        self.process_reads.set(self.process_reads.get() + 1);
        self.processes
            .borrow()
            .get(&process_id)
            .cloned()
            .unwrap_or_default()
    }

    fn icon(&self, window: Handle) -> Option<Arc<Image>> {
        self.icon_reads.set(self.icon_reads.get() + 1);
        self.reads.borrow_mut().push(Read::Icon(window));
        self.icons.borrow().get(&window).cloned()
    }

    fn app_user_model_id(&self, window: Handle) -> Option<String> {
        self.reads.borrow_mut().push(Read::AppId(window));
        self.app_ids.borrow().get(&window).cloned()
    }

    fn monitors(&self) -> Vec<Monitor> {
        self.monitors.borrow().clone()
    }

    fn foreground(&self) -> Option<Handle> {
        self.foreground.get()
    }

    fn input_desktop(&self) -> InputDesktop {
        self.input_desktop.borrow().clone()
    }

    fn own_process_id(&self) -> u32 {
        OWN_PROCESS
    }

    fn own_integrity(&self) -> Option<IntegrityLevel> {
        Some(IntegrityLevel::MEDIUM)
    }
}

pub fn window(handle: u64, title: &str) -> RawWindow {
    rename(Handle(handle), title);
    RawWindow {
        handle: Handle(handle),
        owner: None,
        process_id: u32::try_from(handle).unwrap() + 100,
        thread_id: u32::try_from(handle).unwrap() + 1000,
        class: "AppWindow".to_owned(),
        visible: true,
        cloaked: false,
        enabled: true,
        minimized: false,
        maximized: false,
        style: Default::default(),
        frame: Rect::new(100, 100, 900, 700),
        responding: true,
    }
}

pub fn owned(handle: u64, owner: &RawWindow, title: &str) -> RawWindow {
    RawWindow {
        owner: Some(owner.handle),
        process_id: owner.process_id,
        thread_id: owner.thread_id,
        ..window(handle, title)
    }
}

pub fn monitor(handle: u64, device: &str, rect: Rect, primary: bool) -> Monitor {
    Monitor {
        handle: Handle(handle),
        device: device.to_owned(),
        name: format!("Monitor {device}"),
        rect,
        work_area: rect,
        scale: 1.0,
        primary,
    }
}

pub fn titles(snapshot: &crate::Snapshot) -> Vec<String> {
    snapshot
        .entries
        .iter()
        .map(|entry| entry.title.clone())
        .collect()
}

pub fn entry(id: u64, kind: crate::EntryKind, frame: Rect) -> crate::Entry {
    crate::Entry {
        id: crate::EntryId(id),
        kind,
        handle: Handle(id * 10),
        title: format!("entry {id}"),
        frame,
        responding: true,
        ..crate::Entry::default()
    }
}

pub fn desk(entries: Vec<crate::Entry>) -> crate::Snapshot {
    crate::Snapshot {
        revision: 1,
        monitors: vec![
            monitor(1, "\\\\.\\DISPLAY1", Rect::new(0, 0, 1920, 1080), true),
            monitor(2, "\\\\.\\DISPLAY2", Rect::new(1920, 0, 3840, 1080), false),
        ],
        entries,
        input_desktop: crate::InputDesktop::Default,
    }
}
