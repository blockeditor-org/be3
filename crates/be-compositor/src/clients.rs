use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::unstyled::{DockState, Side, TabId};
use beui::{CursorIcon, Drawing, Rect};

use crate::state::WindowId;

pub const LAUNCHER: TabId = TabId::new(0);
const LAUNCHER_SHARE: f32 = 0.28;

pub fn tab_of(id: WindowId) -> TabId {
    TabId::new(id.0)
}

pub fn window_of(tab: TabId) -> Option<WindowId> {
    (tab != LAUNCHER).then(|| WindowId(tab.value()))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Configure(WindowId),
    Close(WindowId),
    Launch(String),
}

#[derive(Clone)]
pub struct ClientSignals {
    pub title: ReadSignal<String>,
    set_title: WriteSignal<String>,
    pub drawing: ReadSignal<Option<Drawing>>,
    set_drawing: WriteSignal<Option<Drawing>>,
    pub focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    pub painted: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, PartialEq)]
struct View {
    rect: Rect,
    hovered: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            rect: Rect::ZERO,
            hovered: false,
        }
    }
}

struct Inner {
    windows: RefCell<HashMap<WindowId, ClientSignals>>,
    views: RefCell<HashMap<WindowId, View>>,
    commands: RefCell<Vec<Command>>,
    focused: Cell<Option<WindowId>>,
    dock: ReadSignal<DockState>,
    set_dock: WriteSignal<DockState>,
    order: ReadSignal<Vec<WindowId>>,
    set_order: WriteSignal<Vec<WindowId>>,
    cursor: ReadSignal<CursorIcon>,
    set_cursor: WriteSignal<CursorIcon>,
    socket: String,
}

#[derive(Clone)]
pub struct Clients(Rc<Inner>);

impl Clients {
    pub fn new(socket: String) -> Self {
        let (dock, set_dock) = create_signal(DockState::new([LAUNCHER]));
        let (order, set_order) = create_signal(Vec::new());
        let (cursor, set_cursor) = create_signal(CursorIcon::Default);
        Self(Rc::new(Inner {
            windows: RefCell::new(HashMap::new()),
            views: RefCell::new(HashMap::new()),
            commands: RefCell::new(Vec::new()),
            focused: Cell::new(None),
            dock,
            set_dock,
            order,
            set_order,
            cursor,
            set_cursor,
            socket,
        }))
    }

    pub fn socket(&self) -> &str {
        &self.0.socket
    }

    pub fn dock(&self) -> ReadSignal<DockState> {
        self.0.dock.clone()
    }

    pub fn set_dock(&self, state: DockState) {
        self.0.set_dock.set(state);
    }

    pub fn order(&self) -> ReadSignal<Vec<WindowId>> {
        self.0.order.clone()
    }

    pub fn cursor(&self) -> ReadSignal<CursorIcon> {
        self.0.cursor.clone()
    }

    pub fn set_cursor(&self, cursor: CursorIcon) {
        if self.0.cursor.get_untracked() != cursor {
            self.0.set_cursor.set(cursor);
        }
    }

    pub fn signals(&self, id: WindowId) -> Option<ClientSignals> {
        self.0.windows.borrow().get(&id).cloned()
    }

    pub fn title(&self, tab: TabId) -> String {
        match window_of(tab).and_then(|id| self.signals(id)) {
            Some(signals) => {
                let title = signals.title.get();
                if title.is_empty() {
                    "Untitled".to_owned()
                } else {
                    title
                }
            }
            None if tab == LAUNCHER => "Launch".to_owned(),
            None => "Closed".to_owned(),
        }
    }

    pub fn open(&self, id: WindowId) {
        let (title, set_title) = create_signal(String::new());
        let (drawing, set_drawing) = create_signal(None);
        let (focused, set_focused) = create_signal(true);
        self.0.windows.borrow_mut().insert(
            id,
            ClientSignals {
                title,
                set_title,
                drawing,
                set_drawing,
                focused,
                set_focused,
                painted: Rc::new(Cell::new(false)),
            },
        );
        self.0.set_order.update(|order| order.push(id));
        self.show(id);
    }

    pub fn show(&self, id: WindowId) {
        let tab = tab_of(id);
        self.0.set_dock.update(|state| place(state, tab));
        if let Some(signals) = self.signals(id) {
            signals.set_focused.set(true);
        }
    }

    pub fn close(&self, id: WindowId) {
        self.0.windows.borrow_mut().remove(&id);
        self.0.views.borrow_mut().remove(&id);
        if self.0.focused.get() == Some(id) {
            self.0.focused.set(None);
        }
        self.0.set_order.update(|order| order.retain(|window| *window != id));
        let tab = tab_of(id);
        if self.0.dock.get_untracked().contains(tab) {
            self.0.set_dock.update(|state| {
                state.remove(tab);
            });
        }
    }

    pub fn retitle(&self, id: WindowId, title: String) {
        if let Some(signals) = self.signals(id) {
            signals.set_title.set(title);
        }
    }

    pub fn draw(&self, id: WindowId, drawing: Option<Drawing>) {
        if let Some(signals) = self.signals(id) {
            signals.set_drawing.set(drawing);
        }
    }

    pub fn take_painted(&self) -> Vec<WindowId> {
        self.0
            .windows
            .borrow()
            .iter()
            .filter(|(_, signals)| signals.painted.replace(false))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn push(&self, command: Command) {
        self.0.commands.borrow_mut().push(command);
    }

    pub fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut *self.0.commands.borrow_mut())
    }

    pub fn placed(&self, id: WindowId, rect: Rect) {
        let mut views = self.0.views.borrow_mut();
        let view = views.entry(id).or_default();
        let resized = view.rect.size() != rect.size();
        view.rect = rect;
        drop(views);
        if resized {
            self.push(Command::Configure(id));
        }
    }

    pub fn unplaced(&self, id: WindowId) {
        self.0.views.borrow_mut().remove(&id);
    }

    pub fn rect(&self, id: WindowId) -> Option<Rect> {
        self.0.views.borrow().get(&id).map(|view| view.rect)
    }

    pub fn hover(&self, id: WindowId, hovered: bool) {
        self.0.views.borrow_mut().entry(id).or_default().hovered = hovered;
    }

    pub fn hovered(&self) -> Option<WindowId> {
        self.0
            .views
            .borrow()
            .iter()
            .find(|(_, view)| view.hovered)
            .map(|(id, _)| *id)
    }

    pub fn focus(&self, id: WindowId, focused: bool) {
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

fn place(state: &mut DockState, tab: TabId) {
    if state.contains(tab) {
        state.show(tab);
        return;
    }
    let launcher = state.find(LAUNCHER).map(|position| position.leaf);
    let elsewhere = state
        .surfaces()
        .into_iter()
        .flat_map(|surface| state.leaves(surface))
        .find(|leaf| Some(*leaf) != launcher);
    let target = state
        .focused_leaf()
        .filter(|leaf| Some(*leaf) != launcher)
        .or(elsewhere);
    match (target, launcher) {
        (Some(leaf), _) => state.push(leaf, tab),
        (None, Some(launcher)) => {
            state.split(launcher, Side::Right, 1.0 - LAUNCHER_SHARE, vec![tab]);
        }
        (None, None) => state.push_to_focused(tab),
    }
    state.show(tab);
}
