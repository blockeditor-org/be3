use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::unstyled::{DockState, TabId};
use beui::{CursorIcon, Drawing, Rect, Vec2, pos2, vec2};

use crate::state::WindowId;

pub const LAUNCHER: TabId = TabId::new(0);
const CHROME: Vec2 = Vec2::new(0.0, 40.0);
const CASCADE: f32 = 32.0;
const MARGIN: f32 = 24.0;
const CASCADE_STEPS: u32 = 8;

pub fn tab_of(id: WindowId) -> TabId {
    TabId::new(id.0)
}

pub fn window_of(tab: TabId) -> Option<WindowId> {
    (tab != LAUNCHER).then(|| WindowId(tab.value()))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Configure(WindowId),
    Fit(WindowId),
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
    area: Cell<Vec2>,
    fits: RefCell<HashMap<WindowId, Vec2>>,
    mapped: RefCell<Vec<WindowId>>,
    opened: Cell<u32>,
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
            area: Cell::new(vec2(1280.0, 800.0)),
            fits: RefCell::new(HashMap::new()),
            mapped: RefCell::new(Vec::new()),
            opened: Cell::new(0),
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
    }

    pub fn set_area(&self, area: Vec2) {
        self.0.area.set(area);
    }

    pub fn mapped(&self, id: WindowId) -> bool {
        self.0.mapped.borrow().contains(&id)
    }

    pub fn map(&self, id: WindowId, size: Vec2) {
        if self.mapped(id) || self.signals(id).is_none() {
            return;
        }
        self.0.mapped.borrow_mut().push(id);
        let area = self.0.area.get();
        let want = vec2(
            size.x.clamp(1.0, (area.x - 2.0 * MARGIN).max(1.0)),
            size.y
                .clamp(1.0, (area.y - 2.0 * MARGIN - CHROME.y).max(1.0)),
        );
        let step = (self.0.opened.get() % CASCADE_STEPS) as f32 * CASCADE;
        self.0.opened.set(self.0.opened.get() + 1);
        let window = want + CHROME;
        let origin = pos2(
            (MARGIN + step).min((area.x - window.x).max(0.0)),
            (MARGIN + step).min((area.y - window.y).max(0.0)),
        );
        self.0.fits.borrow_mut().insert(id, want);
        let tab = tab_of(id);
        self.0.set_dock.update(|state| {
            state.open_window(Rect::from_min_size(origin, window), vec![tab]);
            state.show(tab);
        });
        self.focus_window(id);
    }

    pub fn show(&self, id: WindowId) {
        let tab = tab_of(id);
        if !self.0.dock.get_untracked().contains(tab) {
            let area = self.0.area.get();
            let size = vec2(area.x * 0.6, area.y * 0.6);
            self.0.mapped.borrow_mut().retain(|window| *window != id);
            self.map(id, size);
            return;
        }
        self.0.set_dock.update(|state| {
            state.show(tab);
            if let Some(surface) = state.find(tab).map(|position| position.surface) {
                state.raise(surface);
            }
        });
        self.focus_window(id);
    }

    fn focus_window(&self, id: WindowId) {
        if let Some(signals) = self.signals(id) {
            signals.set_focused.set(false);
            signals.set_focused.set(true);
        }
    }

    pub fn fit(&self, id: WindowId) -> bool {
        let Some(want) = self.0.fits.borrow_mut().remove(&id) else {
            return false;
        };
        let Some(got) = self.rect(id).map(|rect| rect.size()) else {
            return false;
        };
        let tab = tab_of(id);
        let state = self.0.dock.get_untracked();
        let Some(surface) = state.find(tab).map(|position| position.surface) else {
            return false;
        };
        let Some(rect) = state.window_rect(surface) else {
            return false;
        };
        let size = rect.size() + (want - got);
        if size == rect.size() {
            return false;
        }
        self.0
            .set_dock
            .update(|state| state.set_window_rect(surface, Rect::from_min_size(rect.min, size)));
        true
    }

    pub fn fitting(&self, id: WindowId) -> bool {
        self.0.fits.borrow().contains_key(&id)
    }

    pub fn close(&self, id: WindowId) {
        self.0.windows.borrow_mut().remove(&id);
        self.0.views.borrow_mut().remove(&id);
        self.0.fits.borrow_mut().remove(&id);
        self.0.mapped.borrow_mut().retain(|window| *window != id);
        if self.0.focused.get() == Some(id) {
            self.0.focused.set(None);
        }
        self.0
            .set_order
            .update(|order| order.retain(|window| *window != id));
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
        if self.fitting(id) {
            self.push(Command::Fit(id));
        } else if resized {
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
