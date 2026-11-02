use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::*;
use beui::{CursorIcon, ForwardedInput, Image, NodeId, Rect, Vec2, native::Drawing};

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
    pub launch: Option<u64>,
    pub command: Vec<String>,
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
    pub tag: Option<u64>,
}

#[derive(Clone, PartialEq)]
pub struct Popups {
    pub rect: Rect,
    pub areas: Vec<Rect>,
    pub drawing: Drawing,
}

#[derive(Clone)]
pub struct WindowSignals {
    pub drawing: Signal<Option<Drawing>>,
    pub popups: Signal<Option<Popups>>,
    pub focused: Signal<bool>,
    pub(crate) asks: Signal<u64>,
    pub(crate) typing: Signal<Option<u32>>,
    pub(crate) answered: Rc<Cell<u64>>,
    pub(crate) painted: Rc<Cell<bool>>,
}

#[derive(Clone, Default, PartialEq)]
struct View {
    rect: Option<Rect>,
    catchers: Vec<NodeId>,
    popups: Vec<NodeId>,
    mounts: u32,
}

struct Inner {
    windows: RefCell<HashMap<WindowId, WindowSignals>>,
    scope: ScopeContext,
    views: RefCell<HashMap<WindowId, View>>,
    commands: RefCell<Vec<Command>>,
    input: RefCell<Vec<(WindowId, ForwardedInput)>>,
    fullscreen_requests: RefCell<Vec<(WindowId, bool)>>,
    activate: Cell<Option<WindowId>>,
    focused: Cell<Option<WindowId>>,
    raised: RefCell<HashMap<WindowId, u64>>,
    list: Signal<Vec<WindowInfo>>,
    revision: Cell<u64>,
    cursor: Signal<CursorIcon>,
    idle: Signal<bool>,
}

#[derive(Clone, Copy)]
pub struct Windows(Stored<Inner>);

impl Windows {
    pub fn new() -> Self {
        let list = create_signal(Vec::new());
        let cursor = create_signal(CursorIcon::Default);
        let idle = create_signal(false);
        Self(store(Inner {
            windows: RefCell::new(HashMap::new()),
            scope: owner_scope().expect("windows are made inside a scope"),
            views: RefCell::new(HashMap::new()),
            commands: RefCell::new(Vec::new()),
            input: RefCell::new(Vec::new()),
            fullscreen_requests: RefCell::new(Vec::new()),
            activate: Cell::new(None),
            focused: Cell::new(None),
            raised: RefCell::new(HashMap::new()),
            list,
            revision: Cell::new(0),
            cursor,
            idle,
        }))
    }

    pub fn list(&self) -> ReadSignal<Vec<WindowInfo>> {
        let inner = self.0.value();
        inner.list.read_only()
    }

    pub fn revision(&self) -> u64 {
        let inner = self.0.value();
        inner.revision.get()
    }

    pub fn signals(&self, id: WindowId) -> Option<WindowSignals> {
        let inner = self.0.value();
        inner.windows.borrow().get(&id).cloned()
    }

    pub fn cursor(&self) -> ReadSignal<CursorIcon> {
        let inner = self.0.value();
        inner.cursor.read_only()
    }

    pub fn idle(&self) -> ReadSignal<bool> {
        let inner = self.0.value();
        inner.idle.read_only()
    }

    pub(crate) fn set_idle(&self, idle: bool) {
        let inner = self.0.value();
        inner.idle.set(idle);
    }

    pub fn request_fullscreen(&self, id: WindowId, fullscreen: bool) {
        let inner = self.0.value();
        inner
            .fullscreen_requests
            .borrow_mut()
            .push((id, fullscreen));
    }

    pub(crate) fn take_fullscreen_requests(&self) -> Vec<(WindowId, bool)> {
        let inner = self.0.value();
        std::mem::take(&mut *inner.fullscreen_requests.borrow_mut())
    }

    pub fn activate(&self, id: WindowId) {
        let inner = self.0.value();
        inner.activate.set(Some(id));
    }

    pub(crate) fn take_activation(&self) -> Option<WindowId> {
        let inner = self.0.value();
        inner.activate.take()
    }

    pub(crate) fn raise(&self, id: WindowId) {
        let inner = self.0.value();
        let id = self.blocked_by(id).unwrap_or(id);
        let Some(signals) = self.signals(id) else {
            return;
        };
        signals.asks.update(|asks| *asks += 1);
        *inner.raised.borrow_mut().entry(id).or_default() += 1;
        inner.revision.set(inner.revision.get() + 1);
    }

    pub(crate) fn pressed(&self, id: WindowId) {
        if self.signals(id).is_none() {
            return;
        }
        let inner = self.0.value();
        *inner.raised.borrow_mut().entry(id).or_default() += 1;
        inner.revision.set(inner.revision.get() + 1);
    }

    pub fn raised(&self, id: WindowId) -> u64 {
        let inner = self.0.value();
        inner.raised.borrow().get(&id).copied().unwrap_or_default()
    }

    pub fn blocked_by(&self, id: WindowId) -> Option<WindowId> {
        let inner = self.0.value();
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
            tag: None,
        });
    }

    pub fn run(&self, launch: Launch) {
        self.push(Command::Launch(launch));
    }

    pub(crate) fn set_cursor(&self, cursor: CursorIcon) {
        let inner = self.0.value();
        if inner.cursor.get_untracked() != cursor {
            inner.cursor.set(cursor);
        }
    }

    pub(crate) fn open(&self, id: WindowId) {
        let inner = self.0.value();
        let signals = inner.scope.run(|| {
            let drawing = create_signal(None);
            let popups = create_signal(None);
            let focused = create_signal(false);
            let asks = create_signal(1);
            let typing = create_signal(None);
            WindowSignals {
                drawing,
                popups,
                focused,
                asks,
                typing,
                answered: Rc::new(Cell::new(0)),
                painted: Rc::new(Cell::new(false)),
            }
        });
        inner.windows.borrow_mut().insert(id, signals);
    }

    pub(crate) fn set_typing(&self, typing: Option<(WindowId, u64)>) {
        let inner = self.0.value();
        for (id, signals) in inner.windows.borrow().iter() {
            let asks = typing
                .filter(|(window, _)| window == id)
                .map(|(_, asks)| asks as u32);
            if signals.typing.get_untracked() != asks {
                signals.typing.set(asks);
            }
        }
    }

    pub(crate) fn forget(&self, id: WindowId) {
        let inner = self.0.value();
        inner.windows.borrow_mut().remove(&id);
        inner.views.borrow_mut().remove(&id);
        inner.raised.borrow_mut().remove(&id);
        if inner.focused.get() == Some(id) {
            inner.focused.set(None);
        }
    }

    pub(crate) fn set_list(&self, list: Vec<WindowInfo>) {
        let inner = self.0.value();
        if inner.list.get_untracked() != list {
            inner.revision.set(inner.revision.get() + 1);
            inner.list.set(list);
        }
    }

    pub(crate) fn listed(&self, id: WindowId) -> bool {
        let inner = self.0.value();
        inner.list.get_untracked().iter().any(|info| info.id == id)
    }

    pub(crate) fn draw(&self, id: WindowId, drawing: Option<Drawing>, popups: Option<Popups>) {
        if let Some(signals) = self.signals(id) {
            signals.drawing.set(drawing);
            signals.popups.set(popups);
        }
    }

    pub(crate) fn take_painted(&self) -> Vec<WindowId> {
        let inner = self.0.value();
        inner
            .windows
            .borrow()
            .iter()
            .filter(|(_, signals)| signals.painted.replace(false))
            .map(|(id, _)| *id)
            .collect()
    }

    pub(crate) fn push(&self, command: Command) {
        let inner = self.0.value();
        inner.commands.borrow_mut().push(command);
    }

    pub(crate) fn take_commands(&self) -> Vec<Command> {
        let inner = self.0.value();
        std::mem::take(&mut *inner.commands.borrow_mut())
    }

    pub(crate) fn placed(&self, id: WindowId, rect: Rect) {
        let inner = self.0.value();
        let mut views = inner.views.borrow_mut();
        let view = views.entry(id).or_default();
        let changed = view.rect != Some(rect);
        view.rect = Some(rect);
        drop(views);
        if changed {
            self.push(Command::Configure(id));
        }
    }

    pub(crate) fn mounted(&self, id: WindowId) {
        let inner = self.0.value();
        inner.views.borrow_mut().entry(id).or_default().mounts += 1;
    }

    pub(crate) fn caught_by(&self, id: WindowId, catcher: NodeId) {
        let inner = self.0.value();
        let mut views = inner.views.borrow_mut();
        let catchers = &mut views.entry(id).or_default().catchers;
        if !catchers.contains(&catcher) {
            catchers.push(catcher);
        }
    }

    pub(crate) fn popups_caught_by(&self, id: WindowId, catcher: NodeId) {
        let inner = self.0.value();
        let mut views = inner.views.borrow_mut();
        let popups = &mut views.entry(id).or_default().popups;
        if !popups.contains(&catcher) {
            popups.push(catcher);
        }
    }

    pub(crate) fn caught_at(&self, catcher: NodeId) -> Option<(WindowId, Rect)> {
        let inner = self.0.value();
        inner
            .views
            .borrow()
            .iter()
            .find(|(_, view)| view.catchers.contains(&catcher) || view.popups.contains(&catcher))
            .and_then(|(id, view)| Some((*id, view.rect?)))
    }

    pub(crate) fn unplaced(&self, id: WindowId, nodes: [Option<NodeId>; 2]) {
        let inner = self.0.value();
        let mut views = inner.views.borrow_mut();
        let Some(view) = views.get_mut(&id) else {
            return;
        };
        view.mounts = view.mounts.saturating_sub(1);
        view.catchers.retain(|node| !nodes.contains(&Some(*node)));
        view.popups.retain(|node| !nodes.contains(&Some(*node)));
        if view.mounts == 0 {
            views.remove(&id);
        }
    }

    pub fn rect(&self, id: WindowId) -> Option<Rect> {
        let inner = self.0.value();
        inner.views.borrow().get(&id).and_then(|view| view.rect)
    }

    pub(crate) fn forward(&self, id: WindowId, input: ForwardedInput) {
        let inner = self.0.value();
        inner.input.borrow_mut().push((id, input));
    }

    pub(crate) fn take_input(&self) -> Vec<(WindowId, ForwardedInput)> {
        let inner = self.0.value();
        std::mem::take(&mut *inner.input.borrow_mut())
    }

    pub(crate) fn focus(&self, id: WindowId, focused: bool) {
        let inner = self.0.value();
        if focused && let Some(dialog) = self.blocked_by(id) {
            self.raise(dialog);
        }
        if let Some(signals) = self.signals(id)
            && signals.focused.get_untracked() != focused
        {
            signals.focused.set(focused);
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
        let inner = self.0.value();
        inner.focused.get()
    }
}

impl Default for Windows {
    fn default() -> Self {
        Self::new()
    }
}
