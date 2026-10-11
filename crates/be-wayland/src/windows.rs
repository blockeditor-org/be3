use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::{CursorIcon, Drawing, ForwardedInput, Image, NodeId, Rect, Vec2};

use crate::state::WindowId;

#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub parent: Option<WindowId>,
    pub modal: bool,
    pub size: Vec2,
    pub fullscreen: Option<Rect>,
    pub responding: bool,
    pub icon: Option<Image>,
    pub inhibits_shortcuts: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Command {
    Configure(WindowId),
    Close(WindowId),
    RestoreShortcuts(WindowId),
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
    catcher: Option<NodeId>,
}

struct Inner {
    windows: RefCell<HashMap<WindowId, WindowSignals>>,
    views: RefCell<HashMap<WindowId, View>>,
    commands: RefCell<Vec<Command>>,
    input: RefCell<Vec<(WindowId, ForwardedInput)>>,
    fullscreen_requests: RefCell<Vec<(WindowId, bool)>>,
    activate: Cell<Option<WindowId>>,
    focused: Cell<Option<WindowId>>,
    raised: RefCell<HashMap<WindowId, u64>>,
    list: ReadSignal<Vec<WindowInfo>>,
    set_list: WriteSignal<Vec<WindowInfo>>,
    revision: Cell<u64>,
    cursor: ReadSignal<CursorIcon>,
    set_cursor: WriteSignal<CursorIcon>,
    idle: ReadSignal<bool>,
    set_idle: WriteSignal<bool>,
}

#[derive(Clone)]
pub struct Windows(Rc<Inner>);

impl Windows {
    pub fn new() -> Self {
        let (list, set_list) = create_signal(Vec::new());
        let (cursor, set_cursor) = create_signal(CursorIcon::Default);
        let (idle, set_idle) = create_signal(false);
        Self(Rc::new(Inner {
            windows: RefCell::new(HashMap::new()),
            views: RefCell::new(HashMap::new()),
            commands: RefCell::new(Vec::new()),
            input: RefCell::new(Vec::new()),
            fullscreen_requests: RefCell::new(Vec::new()),
            activate: Cell::new(None),
            focused: Cell::new(None),
            raised: RefCell::new(HashMap::new()),
            list,
            set_list,
            revision: Cell::new(0),
            cursor,
            set_cursor,
            idle,
            set_idle,
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

    pub fn idle(&self) -> ReadSignal<bool> {
        self.0.idle.clone()
    }

    pub(crate) fn set_idle(&self, idle: bool) {
        self.0.set_idle.set(idle);
    }

    pub fn request_fullscreen(&self, id: WindowId, fullscreen: bool) {
        self.0
            .fullscreen_requests
            .borrow_mut()
            .push((id, fullscreen));
    }

    pub(crate) fn take_fullscreen_requests(&self) -> Vec<(WindowId, bool)> {
        std::mem::take(&mut *self.0.fullscreen_requests.borrow_mut())
    }

    pub fn activate(&self, id: WindowId) {
        self.0.activate.set(Some(id));
    }

    pub(crate) fn take_activation(&self) -> Option<WindowId> {
        self.0.activate.take()
    }

    pub(crate) fn raise(&self, id: WindowId) {
        let id = self.blocked_by(id).unwrap_or(id);
        let Some(signals) = self.signals(id) else {
            return;
        };
        signals.set_focused.set(true);
        *self.0.raised.borrow_mut().entry(id).or_default() += 1;
        self.0.revision.set(self.0.revision.get() + 1);
    }

    pub fn raised(&self, id: WindowId) -> u64 {
        self.0.raised.borrow().get(&id).copied().unwrap_or_default()
    }

    pub fn blocked_by(&self, id: WindowId) -> Option<WindowId> {
        self.0
            .list
            .with_untracked(|list| crate::dialog::modal_dialog(list, id))
    }

    pub fn close(&self, id: WindowId) {
        self.push(Command::Close(id));
    }

    pub fn restore_shortcuts(&self) {
        if let Some(id) = self.focused() {
            self.push(Command::RestoreShortcuts(id));
        }
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
        self.0.raised.borrow_mut().remove(&id);
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
        let changed = view.rect != Some(rect);
        view.rect = Some(rect);
        drop(views);
        if changed {
            self.push(Command::Configure(id));
        }
    }

    pub(crate) fn caught_by(&self, id: WindowId, catcher: NodeId) {
        self.0.views.borrow_mut().entry(id).or_default().catcher = Some(catcher);
    }

    pub(crate) fn caught_at(&self, catcher: NodeId) -> Option<(WindowId, Rect)> {
        self.0
            .views
            .borrow()
            .iter()
            .find(|(_, view)| view.catcher == Some(catcher))
            .and_then(|(id, view)| Some((*id, view.rect?)))
    }

    pub(crate) fn unplaced(&self, id: WindowId) {
        self.0.views.borrow_mut().remove(&id);
    }

    pub fn rect(&self, id: WindowId) -> Option<Rect> {
        self.0.views.borrow().get(&id).and_then(|view| view.rect)
    }

    pub(crate) fn forward(&self, id: WindowId, input: ForwardedInput) {
        self.0.input.borrow_mut().push((id, input));
    }

    pub(crate) fn take_input(&self) -> Vec<(WindowId, ForwardedInput)> {
        std::mem::take(&mut *self.0.input.borrow_mut())
    }

    pub(crate) fn focus(&self, id: WindowId, focused: bool) {
        if focused && let Some(dialog) = self.blocked_by(id) {
            self.raise(dialog);
        }
        if let Some(signals) = self.signals(id)
            && signals.focused.get_untracked() != focused
        {
            signals.set_focused.set(focused);
        }
        let was = self.0.focused.get();
        if focused {
            self.0.focused.set(Some(id));
        } else if was == Some(id) {
            self.0.focused.set(None);
        }
        if self.0.focused.get() != was {
            self.0.revision.set(self.0.revision.get() + 1);
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
