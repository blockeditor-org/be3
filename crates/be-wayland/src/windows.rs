use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::{CursorIcon, Drawing, Rect, Vec2};

use crate::state::WindowId;

#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub parent: Option<WindowId>,
    pub size: Vec2,
    pub fullscreen: Option<Rect>,
    pub responding: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    Configure(WindowId),
    Close(WindowId),
    Launch(Launch),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub arguments: Vec<String>,
    pub working_dir: Option<String>,
}

#[derive(Clone)]
pub struct WindowSignals {
    pub drawing: ReadSignal<Option<Drawing>>,
    set_drawing: WriteSignal<Option<Drawing>>,
    pub focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    pub(crate) painted: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct View {
    rect: Option<Rect>,
    hovered: bool,
}

struct Inner {
    windows: RefCell<HashMap<WindowId, WindowSignals>>,
    views: RefCell<HashMap<WindowId, View>>,
    commands: RefCell<Vec<Command>>,
    fullscreen_requests: RefCell<Vec<(WindowId, bool)>>,
    toggle_fullscreen: Cell<bool>,
    focused: Cell<Option<WindowId>>,
    list: ReadSignal<Vec<WindowInfo>>,
    set_list: WriteSignal<Vec<WindowInfo>>,
    revision: Cell<u64>,
    cursor: ReadSignal<CursorIcon>,
    set_cursor: WriteSignal<CursorIcon>,
}

#[derive(Clone)]
pub struct Windows(Rc<Inner>);

impl Windows {
    pub fn new() -> Self {
        let (list, set_list) = create_signal(Vec::new());
        let (cursor, set_cursor) = create_signal(CursorIcon::Default);
        Self(Rc::new(Inner {
            windows: RefCell::new(HashMap::new()),
            views: RefCell::new(HashMap::new()),
            commands: RefCell::new(Vec::new()),
            fullscreen_requests: RefCell::new(Vec::new()),
            toggle_fullscreen: Cell::new(false),
            focused: Cell::new(None),
            list,
            set_list,
            revision: Cell::new(0),
            cursor,
            set_cursor,
        }))
    }

    pub fn list(&self) -> ReadSignal<Vec<WindowInfo>> {
        self.0.list.clone()
    }

    pub fn revision(&self) -> u64 {
        self.0.revision.get()
    }

    pub fn signals(&self, id: WindowId) -> Option<WindowSignals> {
        self.0.windows.borrow().get(&id).cloned()
    }

    pub fn cursor(&self) -> ReadSignal<CursorIcon> {
        self.0.cursor.clone()
    }

    pub fn request_fullscreen(&self, id: WindowId, fullscreen: bool) {
        self.0
            .fullscreen_requests
            .borrow_mut()
            .push((id, fullscreen));
    }

    pub fn toggle_fullscreen(&self) {
        self.0.toggle_fullscreen.set(true);
    }

    pub(crate) fn take_fullscreen_toggle(&self) -> bool {
        self.0.toggle_fullscreen.replace(false)
    }

    pub(crate) fn take_fullscreen_requests(&self) -> Vec<(WindowId, bool)> {
        std::mem::take(&mut *self.0.fullscreen_requests.borrow_mut())
    }

    pub(crate) fn raise(&self, id: WindowId) {
        if let Some(signals) = self.signals(id) {
            signals.set_focused.set(true);
        }
    }

    pub fn close(&self, id: WindowId) {
        self.push(Command::Close(id));
    }

    pub fn launch(&self, line: String) {
        self.run(Launch {
            arguments: vec!["sh".to_owned(), "-c".to_owned(), line],
            working_dir: None,
        });
    }

    pub fn run(&self, launch: Launch) {
        self.push(Command::Launch(launch));
    }

    pub(crate) fn set_cursor(&self, cursor: CursorIcon) {
        if self.0.cursor.get_untracked() != cursor {
            self.0.set_cursor.set(cursor);
        }
    }

    pub(crate) fn open(&self, id: WindowId) {
        let (drawing, set_drawing) = create_signal(None);
        let (focused, set_focused) = create_signal(true);
        self.0.windows.borrow_mut().insert(
            id,
            WindowSignals {
                drawing,
                set_drawing,
                focused,
                set_focused,
                painted: Rc::new(Cell::new(false)),
            },
        );
    }

    pub(crate) fn forget(&self, id: WindowId) {
        self.0.windows.borrow_mut().remove(&id);
        self.0.views.borrow_mut().remove(&id);
        if self.0.focused.get() == Some(id) {
            self.0.focused.set(None);
        }
    }

    pub(crate) fn set_list(&self, list: Vec<WindowInfo>) {
        if self.0.list.get_untracked() != list {
            self.0.revision.set(self.0.revision.get() + 1);
            self.0.set_list.set(list);
        }
    }

    pub(crate) fn listed(&self, id: WindowId) -> bool {
        self.0.list.get_untracked().iter().any(|info| info.id == id)
    }

    pub(crate) fn draw(&self, id: WindowId, drawing: Option<Drawing>) {
        if let Some(signals) = self.signals(id) {
            signals.set_drawing.set(drawing);
        }
    }

    pub(crate) fn take_painted(&self) -> Vec<WindowId> {
        self.0
            .windows
            .borrow()
            .iter()
            .filter(|(_, signals)| signals.painted.replace(false))
            .map(|(id, _)| *id)
            .collect()
    }

    pub(crate) fn push(&self, command: Command) {
        self.0.commands.borrow_mut().push(command);
    }

    pub(crate) fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut *self.0.commands.borrow_mut())
    }

    pub(crate) fn placed(&self, id: WindowId, rect: Rect) {
        let mut views = self.0.views.borrow_mut();
        let view = views.entry(id).or_default();
        let resized = view.rect.map(|rect| rect.size()) != Some(rect.size());
        view.rect = Some(rect);
        drop(views);
        if resized {
            self.push(Command::Configure(id));
        }
    }

    pub(crate) fn unplaced(&self, id: WindowId) {
        self.0.views.borrow_mut().remove(&id);
    }

    pub fn rect(&self, id: WindowId) -> Option<Rect> {
        self.0.views.borrow().get(&id).and_then(|view| view.rect)
    }

    pub(crate) fn hover(&self, id: WindowId, hovered: bool) {
        self.0.views.borrow_mut().entry(id).or_default().hovered = hovered;
    }

    pub(crate) fn hovered(&self) -> Option<WindowId> {
        self.0
            .views
            .borrow()
            .iter()
            .find(|(_, view)| view.hovered)
            .map(|(id, _)| *id)
    }

    pub(crate) fn focus(&self, id: WindowId, focused: bool) {
        if let Some(signals) = self.signals(id)
            && signals.focused.get_untracked() != focused
        {
            signals.set_focused.set(focused);
        }
        if focused {
            self.0.focused.set(Some(id));
        } else if self.0.focused.get() == Some(id) {
            self.0.focused.set(None);
        }
        self.push(Command::Configure(id));
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.0.focused.get()
    }
}

impl Default for Windows {
    fn default() -> Self {
        Self::new()
    }
}
