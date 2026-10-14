use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, Stored, WriteSignal, create_signal, store};
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

#[derive(Clone, PartialEq)]
pub struct Popups {
    pub rect: Rect,
    pub areas: Vec<Rect>,
    pub drawing: Drawing,
}

#[derive(Clone)]
pub struct WindowSignals {
    pub drawing: ReadSignal<Option<Drawing>>,
    set_drawing: WriteSignal<Option<Drawing>>,
    pub popups: ReadSignal<Option<Popups>>,
    set_popups: WriteSignal<Option<Popups>>,
    pub focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    pub(crate) painted: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct View {
    rect: Option<Rect>,
    catcher: Option<NodeId>,
    popups: Option<NodeId>,
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

#[derive(Clone, Copy)]
pub struct Windows(Stored<Inner>);

impl Windows {
    pub fn new() -> Self {
        let (list, set_list) = create_signal(Vec::new());
        let (cursor, set_cursor) = create_signal(CursorIcon::Default);
        let (idle, set_idle) = create_signal(false);
        Self(store(Inner {
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
        let inner = self.0.get();
        inner.list
    }

    pub fn revision(&self) -> u64 {
        let inner = self.0.get();
        inner.revision.get()
    }

    pub fn signals(&self, id: WindowId) -> Option<WindowSignals> {
        let inner = self.0.get();
        inner.windows.borrow().get(&id).cloned()
    }

    pub fn cursor(&self) -> ReadSignal<CursorIcon> {
        let inner = self.0.get();
        inner.cursor
    }

    pub fn idle(&self) -> ReadSignal<bool> {
        let inner = self.0.get();
        inner.idle
    }

    pub(crate) fn set_idle(&self, idle: bool) {
        let inner = self.0.get();
        inner.set_idle.set(idle);
    }

    pub fn request_fullscreen(&self, id: WindowId, fullscreen: bool) {
        let inner = self.0.get();
        inner
            .fullscreen_requests
            .borrow_mut()
            .push((id, fullscreen));
    }

    pub(crate) fn take_fullscreen_requests(&self) -> Vec<(WindowId, bool)> {
        let inner = self.0.get();
        std::mem::take(&mut *inner.fullscreen_requests.borrow_mut())
    }

    pub fn activate(&self, id: WindowId) {
        let inner = self.0.get();
        inner.activate.set(Some(id));
    }

    pub(crate) fn take_activation(&self) -> Option<WindowId> {
        let inner = self.0.get();
        inner.activate.take()
    }

    pub(crate) fn raise(&self, id: WindowId) {
        let inner = self.0.get();
        let id = self.blocked_by(id).unwrap_or(id);
        let Some(signals) = self.signals(id) else {
            return;
        };
        signals.set_focused.set(true);
        *inner.raised.borrow_mut().entry(id).or_default() += 1;
        inner.revision.set(inner.revision.get() + 1);
    }

    pub(crate) fn pressed(&self, id: WindowId) {
        if self.signals(id).is_none() {
            return;
        }
        *self.0.raised.borrow_mut().entry(id).or_default() += 1;
        self.0.revision.set(self.0.revision.get() + 1);
    }

    pub fn raised(&self, id: WindowId) -> u64 {
        let inner = self.0.get();
        inner.raised.borrow().get(&id).copied().unwrap_or_default()
    }

    pub fn blocked_by(&self, id: WindowId) -> Option<WindowId> {
        let inner = self.0.get();
        inner
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
        let inner = self.0.get();
        if inner.cursor.get_untracked() != cursor {
            inner.set_cursor.set(cursor);
        }
    }

    pub(crate) fn open(&self, id: WindowId) {
        let inner = self.0.get();
        let (drawing, set_drawing) = create_signal(None);
        let (popups, set_popups) = create_signal(None);
        let (focused, set_focused) = create_signal(true);
        inner.windows.borrow_mut().insert(
            id,
            WindowSignals {
                drawing,
                set_drawing,
                popups,
                set_popups,
                focused,
                set_focused,
                painted: Rc::new(Cell::new(false)),
            },
        );
    }

    pub(crate) fn forget(&self, id: WindowId) {
        let inner = self.0.get();
        inner.windows.borrow_mut().remove(&id);
        inner.views.borrow_mut().remove(&id);
        inner.raised.borrow_mut().remove(&id);
        if inner.focused.get() == Some(id) {
            inner.focused.set(None);
        }
    }

    pub(crate) fn set_list(&self, list: Vec<WindowInfo>) {
        let inner = self.0.get();
        if inner.list.get_untracked() != list {
            inner.revision.set(inner.revision.get() + 1);
            inner.set_list.set(list);
        }
    }

    pub(crate) fn listed(&self, id: WindowId) -> bool {
        let inner = self.0.get();
        inner.list.get_untracked().iter().any(|info| info.id == id)
    }

    pub(crate) fn draw(&self, id: WindowId, drawing: Option<Drawing>, popups: Option<Popups>) {
        if let Some(signals) = self.signals(id) {
            signals.set_drawing.set(drawing);
            signals.set_popups.set(popups);
        }
    }

    pub(crate) fn take_painted(&self) -> Vec<WindowId> {
        let inner = self.0.get();
        inner
            .windows
            .borrow()
            .iter()
            .filter(|(_, signals)| signals.painted.replace(false))
            .map(|(id, _)| *id)
            .collect()
    }

    pub(crate) fn push(&self, command: Command) {
        let inner = self.0.get();
        inner.commands.borrow_mut().push(command);
    }

    pub(crate) fn take_commands(&self) -> Vec<Command> {
        let inner = self.0.get();
        std::mem::take(&mut *inner.commands.borrow_mut())
    }

    pub(crate) fn placed(&self, id: WindowId, rect: Rect) {
        let inner = self.0.get();
        let mut views = inner.views.borrow_mut();
        let view = views.entry(id).or_default();
        let changed = view.rect != Some(rect);
        view.rect = Some(rect);
        drop(views);
        if changed {
            self.push(Command::Configure(id));
        }
    }

    pub(crate) fn caught_by(&self, id: WindowId, catcher: NodeId) {
        let inner = self.0.get();
        inner.views.borrow_mut().entry(id).or_default().catcher = Some(catcher);
    }

    pub(crate) fn popups_caught_by(&self, id: WindowId, catcher: NodeId) {
        let inner = self.0.get();
        inner.views.borrow_mut().entry(id).or_default().popups = Some(catcher);
    }

    pub(crate) fn caught_at(&self, catcher: NodeId) -> Option<(WindowId, Rect)> {
        let inner = self.0.get();
        inner
            .views
            .borrow()
            .iter()
            .find(|(_, view)| view.catcher == Some(catcher) || view.popups == Some(catcher))
            .and_then(|(id, view)| Some((*id, view.rect?)))
    }

    pub(crate) fn unplaced(&self, id: WindowId) {
        let inner = self.0.get();
        inner.views.borrow_mut().remove(&id);
    }

    pub fn rect(&self, id: WindowId) -> Option<Rect> {
        let inner = self.0.get();
        inner.views.borrow().get(&id).and_then(|view| view.rect)
    }

    pub(crate) fn forward(&self, id: WindowId, input: ForwardedInput) {
        let inner = self.0.get();
        inner.input.borrow_mut().push((id, input));
    }

    pub(crate) fn take_input(&self) -> Vec<(WindowId, ForwardedInput)> {
        let inner = self.0.get();
        std::mem::take(&mut *inner.input.borrow_mut())
    }

    pub(crate) fn focus(&self, id: WindowId, focused: bool) {
        let inner = self.0.get();
        if focused && let Some(dialog) = self.blocked_by(id) {
            self.raise(dialog);
        }
        if let Some(signals) = self.signals(id)
            && signals.focused.get_untracked() != focused
        {
            signals.set_focused.set(focused);
        }
        let was = inner.focused.get();
        if focused {
            inner.focused.set(Some(id));
        } else if was == Some(id) {
            inner.focused.set(None);
        }
        if inner.focused.get() != was {
            inner.revision.set(inner.revision.get() + 1);
        }
        self.push(Command::Configure(id));
    }

    pub fn focused(&self) -> Option<WindowId> {
        let inner = self.0.get();
        inner.focused.get()
    }
}

impl Default for Windows {
    fn default() -> Self {
        Self::new()
    }
}
