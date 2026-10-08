use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::{CursorIcon, Drawing, NodeId, Rect, Vec2};

use crate::state::WindowId;

#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub parent: Option<WindowId>,
    pub size: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fullscreen {
    pub id: WindowId,
    pub insets: Insets,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    Configure(WindowId),
    Close(WindowId),
    Launch(String),
}

#[derive(Clone)]
pub struct WindowSignals {
    pub drawing: ReadSignal<Option<Drawing>>,
    set_drawing: WriteSignal<Option<Drawing>>,
    pub focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    pub(crate) node: ReadSignal<Option<NodeId>>,
    set_node: WriteSignal<Option<NodeId>>,
    pub(crate) painted: Rc<Cell<bool>>,
}

impl WindowSignals {
    pub(crate) fn shown_by(&self, node: Option<NodeId>) {
        self.set_node.set(node);
    }

    pub(crate) fn unshown(&self, node: NodeId) {
        if self.node.get_untracked() == Some(node) {
            self.set_node.set(None);
        }
    }
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
    focused: Cell<Option<WindowId>>,
    list: ReadSignal<Vec<WindowInfo>>,
    set_list: WriteSignal<Vec<WindowInfo>>,
    revision: Cell<u64>,
    cursor: ReadSignal<CursorIcon>,
    set_cursor: WriteSignal<CursorIcon>,
    fullscreen: ReadSignal<Option<Fullscreen>>,
    set_fullscreen: WriteSignal<Option<Fullscreen>>,
}

#[derive(Clone)]
pub struct Windows(Rc<Inner>);

impl Windows {
    pub fn new() -> Self {
        let (list, set_list) = create_signal(Vec::new());
        let (cursor, set_cursor) = create_signal(CursorIcon::Default);
        let (fullscreen, set_fullscreen) = create_signal(None);
        Self(Rc::new(Inner {
            windows: RefCell::new(HashMap::new()),
            views: RefCell::new(HashMap::new()),
            commands: RefCell::new(Vec::new()),
            focused: Cell::new(None),
            list,
            set_list,
            revision: Cell::new(0),
            cursor,
            set_cursor,
            fullscreen,
            set_fullscreen,
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

    pub fn fullscreen(&self) -> ReadSignal<Option<Fullscreen>> {
        self.0.fullscreen.clone()
    }

    pub(crate) fn show_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if self.0.fullscreen.get_untracked() != fullscreen {
            self.0.set_fullscreen.set(fullscreen);
        }
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
        self.push(Command::Launch(line));
    }

    pub(crate) fn set_cursor(&self, cursor: CursorIcon) {
        if self.0.cursor.get_untracked() != cursor {
            self.0.set_cursor.set(cursor);
        }
    }

    pub(crate) fn open(&self, id: WindowId) {
        let (drawing, set_drawing) = create_signal(None);
        let (focused, set_focused) = create_signal(true);
        let (node, set_node) = create_signal(None);
        self.0.windows.borrow_mut().insert(
            id,
            WindowSignals {
                drawing,
                set_drawing,
                focused,
                set_focused,
                node,
                set_node,
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
        if self
            .0
            .fullscreen
            .get_untracked()
            .is_some_and(|fullscreen| fullscreen.id == id)
        {
            self.0.set_fullscreen.set(None);
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
