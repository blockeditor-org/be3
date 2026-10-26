use beui_core::icons::Glyph;
mod dialog;
mod docking;
mod documents;
mod fullscreen;
mod keyboard;
mod modifier_drag;
pub mod state;
mod switch;
#[cfg(test)]
mod tests;

use beui_core::layer::LayerPolicy;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::Choice;
use crate::ChoiceKind;
use crate::ChoiceOption;
use crate::ChoiceOptionHandle;
use crate::DragHandle;
use crate::DragPoint;
use crate::Draggable;
use crate::DropHandle;
use crate::DropTarget;
use crate::Scroll;
use crate::back_slide::BackSlide;
use crate::context_menu::ContextMenu;
use crate::menu::{MenuItem, MenuNodes};
use crate::menu_popup::MenuStyle;
use crate::rubber_band::{Band, WINDOW_SPRING};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::base::{Direction, ItemSize};
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use beui_core::image::Image;
use beui_core::input::{CursorIcon, Key, KeyPress, Modifiers, PointerPress};
use beui_core::node::NodeId;
use beui_core::window::{WindowEdge, WindowId, WindowPlacement, WindowReport};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

use dialog::{DialogParts, DockDialogs};
pub use dialog::{DockDialogHandle, DockDialogPresentation, DockSheetHandle};
pub use docking::{
    DockDialog, DockEntry, DockGroup, DockKey, DockNode, DockPane, DockSplit, DockTab, DockWindow,
    Docking, DockingLayout, DockingSnapshot, OwnedDockingLayout,
};
pub use documents::{
    DockDocument, DockDocumentBody, DockDocumentTabs, DockDocuments, DockOpened, DockSlot,
};
use fullscreen::DockAway;
pub use fullscreen::DockAwayHandle;
use modifier_drag::{Resized, modifier_resize};
pub use state::{
    DockDrop, DockFullscreen, DockLayout, DockSplitter, DockState, DockSwitch, DockTree,
    DockTreeEntry, DockVisit, Entry, GroupId, LeafId, Side, SplitId, SurfaceId, TabId, TabPosition,
    Tree, layout_surface, layout_tree,
};
use state::{FLOATING_SIZE, MIN_WINDOW_SIZE, fraction_moved};
pub use state::{MIN_PANE_LENGTH, MIN_SIDEBAR_WIDTH, SIDEBAR_WIDTH};
use switch::DockSwitchView;
pub use switch::{DockSwitchHandle, DockSwitchRowHandle};

pub const SPLITTER_THICKNESS: f32 = 6.0;
const EDGE_ZONE: f32 = 0.22;
const HANDLE_REACH: f32 = 8.0;
const GRIP_CORNER: f32 = 24.0;
const MARKER_WIDTH: f32 = 3.0;
const SPLIT_STEP: f32 = 0.02;
const SIDEBAR_STEP: f32 = 16.0;
pub const ICON_SIDEBAR_WIDTH: f32 = 40.0;
const SIDEBAR_SECONDS: f32 = 0.18;
const FLOAT_INSET: Vec2 = Vec2::new(64.0, 48.0);
const GRAB_OFFSET: Vec2 = Vec2::new(72.0, 14.0);
const GROUP_ZONE: f32 = 0.3;
const OUTER_EDGE: f32 = 18.0;
const WINDOW_KEEP_VISIBLE: f32 = 96.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DockMode {
    #[default]
    Tiled,
    Stacked,
}

pub struct DockTabHandle {
    pub entry: Entry,
    pub leaf: LeafId,
    pub index: usize,
    pub floating: bool,
    pub pinned: bool,
    pub vertical: bool,
    pub icons_only: Memo<bool>,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub picture: Memo<Option<Image>>,
    pub tabs: Memo<Vec<TabId>>,
    pub has_next: Memo<bool>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub dragged: Memo<bool>,
    pub closable: Memo<bool>,
    pub waiting: Memo<bool>,
    pub close: ClickCallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockDragged {
    Entry(Entry),
    Pane(LeafId),
}

pub struct DockPreviewHandle {
    pub dragged: DockDragged,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
}

pub struct DockChromeHandle {
    pub floating: bool,
    pub barred: bool,
    pub focused: Memo<bool>,
    pub content: Child,
}

pub struct DockBarHandle {
    pub floating: bool,
    pub vertical: bool,
    pub icons_only: Memo<bool>,
    pub focused: Memo<bool>,
    pub grip: NodeId,
    pub tabs: Option<NodeId>,
    pub title: Memo<String>,
    pub vacant: Memo<bool>,
    pub closable: Memo<bool>,
    pub menu: Memo<Vec<MenuNode>>,
    pub close: ClickCallback,
}

pub struct DockSplitterHandle {
    pub direction: Direction,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct DockGripHandle {
    pub floating: bool,
    pub vertical: bool,
    pub focused: Memo<bool>,
}

pub struct DockStackHandle {
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub backward: Memo<bool>,
    pub menu: Memo<Vec<MenuNode>>,
    pub back: ClickCallback,
    pub closable_tabs: Memo<usize>,
    pub switch_label: Memo<String>,
    pub open_switcher: ClickCallback,
}

pub struct DockSwitcherHandle {
    pub open: ReadSignal<bool>,
    pub close: ClickCallback,
    pub tabs: Memo<Vec<TabId>>,
    pub fixed: Memo<Vec<TabId>>,
    pub card: Func<TabId, DockSwitcherCardHandle>,
}

pub struct DockSwitcherCardHandle {
    pub tab: TabId,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub current: Memo<bool>,
    pub closable: bool,
    pub menu: Memo<Vec<MenuNode>>,
    pub pick: ClickCallback,
    pub close: ClickCallback,
}

#[derive(Clone, Copy)]
pub struct DockTabControl {
    tab: TabId,
    state: Binding<DockState>,
    dock: State,
}

impl DockTabControl {
    fn dock(&self) -> Option<State> {
        self.dock.panels.is_alive().then_some(self.dock)
    }

    pub fn tab(&self) -> TabId {
        self.tab
    }

    pub fn fullscreen(&self) -> bool {
        self.state.with(|state| {
            state
                .fullscreen()
                .is_some_and(|shown| shown.tab == self.tab)
        })
    }

    pub fn enter_fullscreen(&self, area: Option<Rect>) {
        let tab = self.tab;
        if let Some(dock) = self.dock() {
            dock.edit(|state| state.set_fullscreen(Some(DockFullscreen { tab, area })));
        }
    }

    pub fn leave_fullscreen(&self) {
        if let Some(dock) = self.dock() {
            dock.leave_fullscreen(self.tab);
        }
    }

    pub fn offer_fullscreen(&self) {
        let Some(dock) = self.dock() else {
            return;
        };
        let tab = self.tab;
        if untrack(|| dock.is_dialog(tab)) {
            return;
        }
        let key = NEXT_MENU.with(|next| next.replace(next.get() + 1));
        let offers = dock.offers;
        offers.update(|offers| {
            offers.insert(tab, key);
        });
        on_cleanup(move || {
            offers.update(|offers| {
                if offers.get(&tab) == Some(&key) {
                    offers.remove(&tab);
                }
            });
        });
    }

    pub fn focused(&self) -> bool {
        let Some(dock) = self.dock() else {
            return self
                .state
                .with(|state| state.focused_tab() == Some(self.tab));
        };
        match dock.dialog_focus.get() {
            Some(dialog) => dialog == self.tab,
            None => self
                .state
                .with(|state| state.focused_tab() == Some(self.tab)),
        }
    }

    pub fn attention(&self) -> Option<u64> {
        let dock = self.dock()?;
        dock.attention
            .with(|(count, target)| (*target == Some(self.tab)).then_some(*count))
    }

    pub fn show(&self) {
        let Some(dock) = self.dock() else {
            return;
        };
        match dock.is_dialog(self.tab) {
            true => dock.focus_dialog(self.tab),
            false => {
                dock.show(self.tab);
                dock.release_dialog_focus();
            }
        }
    }

    pub fn resize_window(&self, size: Vec2) {
        let tab = self.tab;
        let Some(dock) = self.dock() else {
            return;
        };
        dock.edit(|state| {
            let Some(surface) = state.find(tab).map(|position| position.surface) else {
                return;
            };
            if state.surface_tabs(surface) != [tab] {
                return;
            }
            if let Some(rect) = state.window_rect(surface) {
                state.set_window_rect(surface, Rect::from_min_size(rect.min, size));
            }
        });
    }
}

pub fn use_dock_tab() -> Option<DockTabControl> {
    use_context::<DockTabControl>()
}

pub type DockMenus = HashMap<TabId, (u64, Memo<Vec<MenuNode>>)>;

#[derive(Clone, Copy)]
pub struct DockTabMenu {
    pub tab: TabId,
    pub menu: Signal<DockMenus>,
}

thread_local! {
    static NEXT_MENU: Cell<u64> = const { Cell::new(1) };
}

pub fn dock_menu(items: Memo<Vec<MenuNode>>) {
    let Some(DockTabMenu { tab, menu }) = use_context::<DockTabMenu>() else {
        return;
    };
    let key = NEXT_MENU.with(|next| next.replace(next.get() + 1));
    menu.update(|all| {
        all.insert(tab, (key, items));
    });
    on_cleanup(move || {
        menu.update(|all| {
            if all.get(&tab).is_some_and(|(held, _)| *held == key) {
                all.remove(&tab);
            }
        });
    });
}

pub fn dock_menu_items(menus: ReadSignal<DockMenus>, tab: Option<TabId>) -> Vec<MenuNode> {
    let Some(tab) = tab else {
        return Vec::new();
    };
    menus
        .with(|menus| menus.get(&tab).map(|(_, items)| *items))
        .map(|items| items.get())
        .unwrap_or_default()
}

#[derive(Clone, Copy, Default)]
struct TabBar {
    strip: NodeRef,
    tabs: NodeRef,
}

#[derive(Clone, Copy, PartialEq)]
struct Drag {
    dragged: DockDragged,
    target: Option<DockDrop>,
    highlight: Option<Rect>,
}

#[derive(Clone, Copy)]
struct State {
    state: Binding<DockState>,
    on_close: Callback<TabId>,
    drag: Signal<Option<Drag>>,
    title: Func<TabId, String>,
    group_title: Func<GroupId, Option<String>>,
    icon: Func<TabId, Option<Glyph>>,
    picture: Func<TabId, Option<Image>>,
    closable: Func<TabId, bool>,
    menu: MenuStyle,
    drag_modifier: Memo<Option<Modifiers>>,
    menus: Signal<DockMenus>,
    offers: Signal<HashMap<TabId, u64>>,
    thickness: f32,
    reach: f32,
    group_inset: f32,
    icon_sidebar_width: f32,
    rect: ReadSignal<Rect>,
    opens: ReadSignal<bool>,
    home: Option<WindowHandle>,
    placements: Signal<HashMap<SurfaceId, WindowPlacement>>,
    panes: Stored<RefCell<HashMap<Tree, NodeRef>>>,
    windows: Stored<RefCell<HashMap<SurfaceId, NodeRef>>>,
    fullscreen_layer: NodeRef,
    bars: Stored<RefCell<HashMap<LeafId, TabBar>>>,
    panels: Stored<RefCell<HashMap<TabId, (NodeId, Scope)>>>,
    owner: Stored<Option<ScopeContext>>,
    tab: RenderFn<DockTabHandle>,
    content: RenderFn<TabId>,
    empty: RenderFn<LeafId>,
    fullscreen: RenderFn<NodeId>,
    away: RenderFn<DockAwayHandle>,
    chrome: RenderFn<DockChromeHandle>,
    bar: RenderFn<DockBarHandle>,
    splitter: RenderFn<DockSplitterHandle>,
    grip: RenderFn<DockGripHandle>,
    highlight: RenderFn<()>,
    preview: RenderFn<DockPreviewHandle>,
    stack: Option<RenderFn<DockStackHandle>>,
    switcher: Option<RenderFn<DockSwitcherHandle>>,
    switch: Option<RenderFn<DockSwitchHandle>>,
    mode: Memo<DockMode>,
    dialogs: Memo<Vec<(TabId, TabId)>>,
    dialog: Func<TabId, Option<DialogParts>>,
    dialog_chrome: RenderFn<DockDialogHandle>,
    dialog_sheet: Option<RenderFn<DockSheetHandle>>,
    dialog_scrim: Memo<Color32>,
    dialog_margin: u16,
    dialog_gap: u16,
    dialog_frames: Stored<RefCell<HashMap<TabId, NodeRef>>>,
    dialog_bodies: Stored<RefCell<HashMap<TabId, NodeRef>>>,
    dialog_focus: Signal<Option<TabId>>,
    attention: Signal<(u64, Option<TabId>)>,
    given: Signal<(u64, Option<TabId>)>,
    remembered: Stored<RefCell<HashMap<TabId, NodeId>>>,
}

impl State {
    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next == current {
            return;
        }
        self.state.set(next);
    }

    fn title(&self, tab: TabId) -> String {
        self.title.call(tab)
    }

    fn all_closable(&self, contents: Memo<Vec<TabId>>) -> Memo<bool> {
        let closable = self.closable;
        create_memo(move || {
            contents.with(|tabs| !tabs.is_empty() && tabs.iter().all(|tab| closable.call(*tab)))
        })
    }

    fn icon(&self, tab: TabId) -> Option<Glyph> {
        self.icon.call(tab)
    }

    fn entry_picture(&self, entry: Entry) -> Option<Image> {
        match entry {
            Entry::Tab(tab) => self.picture.call(tab),
            Entry::Group(_) => None,
        }
    }

    fn entry_icon(&self, entry: Entry) -> Option<Glyph> {
        match entry {
            Entry::Tab(tab) => self.icon(tab),
            Entry::Group(_) => None,
        }
    }

    fn dragged_icon(&self, dragged: DockDragged) -> Option<Glyph> {
        match dragged {
            DockDragged::Entry(entry) => self.entry_icon(entry),
            DockDragged::Pane(leaf) => {
                match self.state.with(|state| state.entries(leaf)).as_slice() {
                    [entry] => self.entry_icon(*entry),
                    _ => None,
                }
            }
        }
    }

    fn entry_title(&self, entry: Entry) -> String {
        match entry {
            Entry::Tab(tab) => self.title(tab),
            Entry::Group(group) => {
                if let Some(title) = self.group_title.call(group) {
                    return title;
                }
                let tabs = self.state.with(|state| state.group_tabs(group));
                group_title(tabs.into_iter().map(|tab| self.title(tab)).collect())
            }
        }
    }

    fn leaf_menu(&self, leaf: Option<LeafId>) -> Memo<Vec<MenuNode>> {
        let dock = *self;
        create_memo(move || {
            let tab = leaf.and_then(|leaf| {
                dock.state
                    .with(|state| state.active_entry(leaf))
                    .and_then(Entry::tab)
            });
            dock_menu_items(dock.menus.read_only(), tab)
        })
    }

    fn panel(&self, tab: TabId) -> NodeId {
        if let Some((panel, _)) = self.panels.value().borrow().get(&tab) {
            return *panel;
        }
        let scope = self.open_scope();
        let menus = self.menus;
        let control = DockTabControl {
            tab,
            state: self.state,
            dock: *self,
        };
        let panel = scope.context().run(|| {
            provide_context(DockTabMenu { tab, menu: menus });
            provide_context(control);
            self.content.call(tab)
        });
        self.panels.value().borrow_mut().insert(tab, (panel, scope));
        panel
    }

    fn open_scope(&self) -> Scope {
        self.owner
            .with(|owner| owner.as_ref().and_then(ScopeContext::child))
            .unwrap_or_else(Scope::detached)
    }

    fn keep_panels(&self, tabs: &[TabId]) {
        let dropped: Vec<TabId> = self
            .panels
            .value()
            .borrow()
            .keys()
            .filter(|tab| !tabs.contains(tab))
            .copied()
            .collect();
        for tab in dropped {
            self.forget_panel(tab);
        }
    }

    fn forget_panel(&self, tab: TabId) {
        self.remembered.value().borrow_mut().remove(&tab);
        let panel = self.panels.value().borrow_mut().remove(&tab);
        drop(panel);
    }

    fn pane_rect(&self, tree: Tree) -> Option<Rect> {
        let node = self.panes.value().borrow().get(&tree)?.try_get()?;
        placements().rect(node)
    }

    fn raise_window(&self, window: NodeRef) {
        let Some(window) = window.try_get() else {
            return;
        };
        let fullscreen = self
            .fullscreen_layer
            .try_get()
            .filter(|_| self.fullscreen_in_front());
        window.raise_overlay();
        if let Some(fullscreen) = fullscreen {
            fullscreen.raise_overlay();
        }
    }

    fn surface_node(&self, surface: SurfaceId) -> Option<NodeId> {
        let window = self
            .windows
            .value()
            .borrow()
            .get(&surface)
            .and_then(NodeRef::try_get);
        window.or_else(|| {
            self.panes
                .value()
                .borrow()
                .get(&Tree::Surface(surface))?
                .try_get()
        })
    }

    fn surfaces_at(&self, state: &DockState, window: WindowId, pos: Pos2) -> Vec<SurfaceId> {
        let surfaces: Vec<(NodeId, SurfaceId)> = state
            .surfaces()
            .into_iter()
            .filter_map(|surface| Some((self.surface_node(surface)?, surface)))
            .collect();
        let nodes: Vec<NodeId> = surfaces.iter().map(|(node, _)| *node).collect();
        stacked_at(&nodes, window, pos)
            .into_iter()
            .filter_map(|node| {
                surfaces
                    .iter()
                    .find(|(held, _)| *held == node)
                    .map(|(_, surface)| *surface)
            })
            .collect()
    }

    fn surface_rect(&self, surface: SurfaceId) -> Option<Rect> {
        let window = self
            .windows
            .value()
            .borrow()
            .get(&surface)
            .and_then(NodeRef::try_get);
        match window {
            Some(window) => placements().rect(window),
            None => self.pane_rect(Tree::Surface(surface)),
        }
    }

    fn tab_rects(&self, leaf: LeafId) -> Vec<Rect> {
        let Some(node) = self
            .bars
            .value()
            .borrow()
            .get(&leaf)
            .and_then(|bar| bar.tabs.try_get())
        else {
            return Vec::new();
        };
        let placements = placements();
        node.children()
            .into_iter()
            .filter_map(|child| placements.rect(child))
            .collect()
    }

    fn over_window_bar(&self, surface: SurfaceId, pos: Pos2) -> bool {
        let Some(pane) = self.pane_rect(Tree::Surface(surface)) else {
            return false;
        };
        if pane.contains(pos) {
            return false;
        }
        let leaves = self.state.get_untracked().leaves(surface);
        !leaves
            .into_iter()
            .flat_map(|leaf| self.tab_rects(leaf))
            .any(|tab| tab.contains(pos))
    }

    fn bar_rect(&self, leaf: LeafId) -> Option<Rect> {
        let node = self.bars.value().borrow().get(&leaf)?.strip.try_get()?;
        placements().rect(node)
    }

    fn toggle_vertical(&self, leaf: LeafId) {
        self.edit(|state| {
            let vertical = state.is_vertical(leaf);
            state.set_vertical(leaf, !vertical);
        });
    }

    fn pane_test_id(&self, prefix: &str, leaf: LeafId) -> String {
        match self
            .state
            .with_untracked(|state| state.pane_key(leaf).map(str::to_owned))
        {
            Some(key) => format!("{prefix}.{key}"),
            None => prefix.to_owned(),
        }
    }

    fn toggle_icons_only(&self, leaf: LeafId) {
        self.edit(|state| {
            let icons_only = state.is_icons_only(leaf);
            state.set_icons_only(leaf, !icons_only);
        });
    }

    fn icons_only(&self, leaf: Option<LeafId>) -> Memo<bool> {
        let state = self.state;
        create_memo(move || leaf.is_some_and(|leaf| state.with(|state| state.is_icons_only(leaf))))
    }

    fn sidebar_target(&self, leaf: Option<LeafId>) -> Memo<f32> {
        let state = self.state;
        let narrow = self.icon_sidebar_width;
        create_memo(move || match leaf {
            Some(leaf) => state.with(|state| match state.is_icons_only(leaf) {
                true => narrow,
                false => state.sidebar_width(leaf),
            }),
            None => SIDEBAR_WIDTH,
        })
    }

    fn sidebar_width(&self, leaf: Option<LeafId>) -> Memo<f32> {
        let icons_only = self.icons_only(leaf);
        let target = self.sidebar_target(leaf);
        let shown = create_signal(target.get_untracked());
        let from = StoredCell::new(shown.get_untracked());
        let animation = create_animation(
            move |frame| {
                let progress = tween(
                    frame.elapsed.as_secs_f32(),
                    SIDEBAR_SECONDS,
                    Easing::OutCubic,
                );
                let start = from.get();
                shown.set(start + (target.get_untracked() - start) * progress);
                progress < 1.0
            },
            move || shown.set(target.get_untracked()),
        );
        let was = Cell::new(icons_only.get_untracked());
        create_effect(move || {
            let width = target.get();
            if icons_only.get() != was.replace(icons_only.get()) {
                from.set(shown.get_untracked());
                animation.stop();
                animation.start();
            } else if !animation.running() {
                shown.set(width);
            }
        });
        create_memo(move || shown.get())
    }

    fn close_tab(&self, tab: TabId) {
        self.on_close.call(tab);
    }

    fn leave_fullscreen(&self, tab: TabId) {
        self.edit(|state| {
            if state.fullscreen().is_some_and(|shown| shown.tab == tab) {
                state.set_fullscreen(None);
            }
        });
    }

    fn toggle_fullscreen(&self, tab: TabId) {
        self.edit(|state| {
            let fullscreen = match state.fullscreen() {
                Some(shown) if shown.tab == tab => None,
                _ => Some(DockFullscreen { tab, area: None }),
            };
            state.set_fullscreen(fullscreen);
        });
    }

    fn show(&self, tab: TabId) {
        self.edit(|state| state.show(tab));
    }

    fn closes_going_back(&self, visit: DockVisit) -> bool {
        visit.opened && self.closable.call(visit.to)
    }

    fn go_back(&self) {
        let Some(visit) = self.state.with_untracked(DockState::back) else {
            return;
        };
        match self.closes_going_back(visit) {
            true => self.close_tab(visit.to),
            false => self.edit(|state| {
                state.go_back();
            }),
        }
    }

    fn close_entry(&self, entry: Entry) {
        if let Entry::Group(group) = entry
            && self.state.with_untracked(|state| state.is_pinned(group))
        {
            return;
        }
        let tabs = self.state.with_untracked(|state| state.entry_tabs(entry));
        for tab in tabs {
            self.close_tab(tab);
        }
    }

    fn float_entry(&self, entry: Entry) {
        self.edit(|state| {
            state.drop_entry(
                entry,
                DockDrop::Window {
                    pos: self.float_origin(),
                },
            )
        });
    }

    fn home(&self) -> WindowId {
        self.home
            .and_then(|window| window.real_window())
            .unwrap_or(WindowId::MAIN)
    }

    fn float_origin(&self) -> Pos2 {
        let dock = self.rect.get_untracked();
        let origin = pos2(FLOAT_INSET.x, FLOAT_INSET.y);
        self.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), dock.size())
    }

    fn drop_outside(&self, dragged: DockDragged) {
        if !self.opens.get_untracked() {
            return;
        }
        let target = DockDrop::Window {
            pos: self.float_origin(),
        };
        let before = self.state.with_untracked(DockState::windows);
        self.edit(|state| match dragged {
            DockDragged::Entry(entry) => state.drop_entry(entry, target),
            DockDragged::Pane(leaf) => state.drop_leaf(leaf, target),
        });
        let opened: Vec<SurfaceId> = self
            .state
            .with_untracked(DockState::windows)
            .into_iter()
            .filter(|surface| !before.contains(surface))
            .collect();
        self.placements.update(|placements| {
            for surface in opened {
                placements.insert(surface, WindowPlacement::AtPointer);
            }
        });
    }

    fn placement(&self, surface: SurfaceId, parent: WindowId) -> WindowPlacement {
        if let Some(placement) = self
            .placements
            .with(|placements| placements.get(&surface).cloned())
        {
            return placement;
        }
        let rect = self
            .state
            .with(|state| state.window_rect(surface))
            .unwrap_or(Rect::ZERO);
        WindowPlacement::Over {
            window: parent,
            at: self.rect.get().min + rect.min.to_vec2() - Vec2::splat(HANDLE_REACH),
        }
    }

    fn window_reported(&self, surface: SurfaceId, report: WindowReport) {
        match report {
            WindowReport::Resized { size, .. } => self.edit(|state| {
                if let Some(rect) = state.window_rect(surface) {
                    let inner = (size - Vec2::splat(HANDLE_REACH * 2.0)).max(Vec2::ZERO);
                    state.set_window_rect(surface, Rect::from_min_size(rect.min, inner));
                }
            }),
            WindowReport::Placed { placement, .. } => self.placements.update(|placements| {
                placements.insert(surface, WindowPlacement::Host(placement));
            }),
            WindowReport::Focused { focused: true, .. } => self.window_focused(surface),
            WindowReport::Scaled { .. } | WindowReport::Focused { .. } => {}
            WindowReport::Closed(_) => {
                for tab in self.state.get_untracked().surface_tabs(surface) {
                    self.close_tab(tab);
                }
            }
        }
    }

    fn window_focused(&self, surface: SurfaceId) {
        let frame = self
            .windows
            .value()
            .borrow()
            .get(&surface)
            .and_then(|frame| frame.try_get());
        if frame.is_none_or(|frame| frame.focus_is_within()) {
            return;
        }
        let leaf = self.state.with_untracked(|state| {
            state
                .focused_leaf()
                .filter(|leaf| state.surface_of(*leaf) == Some(surface))
                .or_else(|| state.leaves(surface).first().copied())
        });
        let Some(leaf) = leaf else {
            return;
        };
        if self.state.with_untracked(|state| state.focused_leaf()) != Some(leaf) {
            self.edit(|state| state.focus(leaf));
        }
        if let Some(tab) = self.state.with_untracked(|state| state.active_tab(leaf)) {
            self.give_keyboard(tab);
        }
    }

    fn clamped_origin(&self, rect: Rect, bounds: Vec2) -> Pos2 {
        let x = rect.min.x.clamp(0.0, (bounds.x - rect.width()).max(0.0));
        let y = rect.min.y.clamp(0.0, (bounds.y - rect.height()).max(0.0));
        pos2(x, y)
    }

    fn cycle(&self, backwards: bool) -> bool {
        let state = self.state.get_untracked();
        let Some(leaf) = state.focused_leaf() else {
            return false;
        };
        let count = state.entries(leaf).len();
        if count < 2 {
            return false;
        }
        let index = state.active_index(leaf);
        let next = match backwards {
            true => (index + count - 1) % count,
            false => (index + 1) % count,
        };
        self.edit(|state| state.set_active_index(leaf, next));
        true
    }

    fn dragged_title(&self, dragged: DockDragged) -> String {
        match dragged {
            DockDragged::Entry(entry) => self.entry_title(entry),
            DockDragged::Pane(leaf) => {
                let tabs = self.state.with(|state| {
                    state
                        .entries(leaf)
                        .into_iter()
                        .flat_map(|entry| state.entry_tabs(entry))
                        .collect::<Vec<_>>()
                });
                group_title(tabs.into_iter().map(|tab| self.title(tab)).collect())
            }
        }
    }

    fn close_leaf(&self, leaf: LeafId) {
        let entries = self.state.with_untracked(|state| state.entries(leaf));
        for entry in entries {
            self.close_entry(entry);
        }
    }

    fn begin_drag(&self, dragged: DockDragged) {
        self.drag.set(Some(Drag {
            dragged,
            target: None,
            highlight: None,
        }));
    }

    fn drag_over(&self, point: Option<DragPoint>) {
        let Some(mut drag) = self.drag.get_untracked() else {
            return;
        };
        let (target, highlight) = match point {
            Some(point) => self.admitted(point, drag.dragged),
            None => (None, None),
        };
        drag.target = target;
        drag.highlight = highlight;
        self.drag.set(Some(drag));
    }

    fn end_drag(&self) {
        self.drag.set(None);
    }

    fn drop_at(&self, dragged: DockDragged, point: DragPoint) {
        let (target, _) = self.admitted(point, dragged);
        let Some(target) = target else {
            return;
        };
        self.edit(|state| match dragged {
            DockDragged::Entry(entry) => state.drop_entry(entry, target),
            DockDragged::Pane(leaf) => state.drop_leaf(leaf, target),
        });
    }

    fn admitted(&self, point: DragPoint, dragged: DockDragged) -> (Option<DockDrop>, Option<Rect>) {
        let (target, highlight) = self.resolve(point, dragged);
        let refused = target.is_some_and(|target| {
            self.state.with_untracked(|state| match dragged {
                DockDragged::Entry(entry) => !state.admits(entry, target),
                DockDragged::Pane(leaf) => !state.admits_leaf(leaf, target),
            })
        });
        match refused {
            true => (None, None),
            false => (target, highlight),
        }
    }

    fn resolve(&self, point: DragPoint, dragged: DockDragged) -> (Option<DockDrop>, Option<Rect>) {
        let pos = point.pos;
        let state = self.state.get_untracked();
        if !point.modifiers.alt {
            for surface in self.surfaces_at(&state, point.window, pos) {
                let Some(within) = self.surface_rect(surface) else {
                    continue;
                };
                if !within.contains(pos) {
                    continue;
                }
                let split = state.window_rect(surface).is_none();
                if let Some(found) =
                    self.resolve_in(&state, Tree::Surface(surface), pos, split, dragged)
                {
                    return found;
                }
            }
        }
        if point.window != self.home() {
            return (None, None);
        }
        let dock = self.rect.get_untracked();
        let origin = pos - dock.min.to_vec2() - GRAB_OFFSET;
        let origin = self.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), dock.size());
        (
            Some(DockDrop::Window { pos: origin }),
            Some(Rect::from_min_size(
                dock.min + origin.to_vec2(),
                FLOATING_SIZE,
            )),
        )
    }

    fn resolve_in(
        &self,
        state: &DockState,
        tree: Tree,
        pos: Pos2,
        split: bool,
        dragged: DockDragged,
    ) -> Option<(Option<DockDrop>, Option<Rect>)> {
        let area = self.pane_rect(tree)?;
        let layout = layout_tree(state, tree, area, self.thickness);
        let over_bar = layout
            .leaves
            .iter()
            .map(|(leaf, _)| *leaf)
            .find(|leaf| self.bar_rect(*leaf).is_some_and(|bar| bar.contains(pos)));
        if let Some(leaf) = over_bar {
            let (target, marker) = self.bar_target(state, leaf, pos, dragged);
            return Some((Some(target), Some(marker)));
        }
        let leaf = nearest_leaf(&layout, pos)?;
        let rect = layout.leaf_rect(leaf).unwrap_or(area);
        let bar = !state.is_vertical(leaf)
            && self
                .bar_rect(leaf)
                .is_some_and(|bar| pos.y >= bar.top() && pos.y <= bar.bottom());
        if bar {
            let (target, marker) = self.bar_target(state, leaf, pos, dragged);
            return Some((Some(target), Some(marker)));
        }
        let hosted = state.active_entry(leaf).and_then(Entry::group);
        if let Some(group) = hosted
            && self
                .pane_rect(Tree::Group(group))
                .is_some_and(|inner| inner.contains(pos))
        {
            if let Some(side) = edge(rect, pos).filter(|_| split) {
                return Some((
                    Some(DockDrop::Split { leaf, side }),
                    Some(side_rect(rect, side)),
                ));
            }
            if let Some(found) = self.resolve_in(state, Tree::Group(group), pos, true, dragged) {
                return Some(found);
            }
        }
        Some(match zone(rect, pos).filter(|_| split) {
            Some(side) => (
                Some(DockDrop::Split { leaf, side }),
                Some(side_rect(rect, side)),
            ),
            None => (Some(DockDrop::Pane { leaf }), Some(rect)),
        })
    }

    fn bar_target(
        &self,
        state: &DockState,
        leaf: LeafId,
        pos: Pos2,
        dragged: DockDragged,
    ) -> (DockDrop, Rect) {
        let bar = self.bar_rect(leaf).unwrap_or(Rect::ZERO);
        let rects = self.tab_rects(leaf);
        let entries = state.entries(leaf);
        let direction = match state.is_vertical(leaf) {
            true => Direction::Vertical,
            false => Direction::Horizontal,
        };
        let along = |point: Pos2| direction.main(point.to_vec2());
        for (index, rect) in rects.iter().enumerate() {
            let across = (along(pos) - along(rect.min)) / direction.main(rect.size()).max(1.0);
            let onto = entries
                .get(index)
                .is_some_and(|onto| DockDragged::Entry(*onto) != dragged)
                && matches!(dragged, DockDragged::Entry(_));
            if onto && (GROUP_ZONE..=1.0 - GROUP_ZONE).contains(&across) {
                return (DockDrop::Group { leaf, index }, *rect);
            }
            if along(pos) < along(rect.center()) {
                let at = match index.checked_sub(1).and_then(|before| rects.get(before)) {
                    Some(before) => (along(before.max) + along(rect.min)) / 2.0,
                    None => along(rect.min),
                };
                return (
                    DockDrop::Tab { leaf, index },
                    marker_rect(direction, at, bar),
                );
            }
        }
        let end = rects.last().map_or(along(bar.min), |rect| along(rect.max));
        (
            DockDrop::Tab {
                leaf,
                index: rects.len(),
            },
            marker_rect(direction, end, bar),
        )
    }

    fn splitter_of(&self, tree: Tree, split: SplitId) -> Option<DockSplitter> {
        let area = self.pane_rect(tree)?;
        let state = self.state.get_untracked();
        layout_tree(&state, tree, area, self.thickness).splitter(split)
    }
}

pub fn sidebar_size(vertical: bool, width: Memo<f32>) -> Prop<ItemSize> {
    match vertical {
        true => width.into_prop().map(ItemSize::Fixed),
        false => ItemSize::Intrinsic.into_prop(),
    }
}

fn group_title(titles: Vec<String>) -> String {
    match titles.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first}, {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} +{}", rest.len()),
    }
}

fn edge(rect: Rect, pos: Pos2) -> Option<Side> {
    let distances = [
        (pos.x - rect.left(), Side::Left),
        (rect.right() - pos.x, Side::Right),
        (pos.y - rect.top(), Side::Above),
        (rect.bottom() - pos.y, Side::Below),
    ];
    distances
        .into_iter()
        .filter(|(distance, _)| *distance < OUTER_EDGE)
        .min_by(|(first, _), (second, _)| first.total_cmp(second))
        .map(|(_, side)| side)
}

fn marker_rect(direction: Direction, at: f32, bar: Rect) -> Rect {
    match direction {
        Direction::Horizontal => {
            let x = at.clamp(bar.left(), bar.right());
            Rect::from_min_max(
                pos2(x - MARKER_WIDTH / 2.0, bar.top()),
                pos2(x + MARKER_WIDTH / 2.0, bar.bottom()),
            )
        }
        Direction::Vertical => {
            let y = at.clamp(bar.top(), bar.bottom());
            Rect::from_min_max(
                pos2(bar.left(), y - MARKER_WIDTH / 2.0),
                pos2(bar.right(), y + MARKER_WIDTH / 2.0),
            )
        }
    }
}

fn nearest_leaf(layout: &DockLayout, pos: Pos2) -> Option<LeafId> {
    layout.leaf_at(pos).or_else(|| {
        layout
            .leaves
            .iter()
            .min_by(|(_, first), (_, second)| {
                let first = first.center().distance(pos);
                let second = second.center().distance(pos);
                first.total_cmp(&second)
            })
            .map(|(leaf, _)| *leaf)
    })
}

fn zone(rect: Rect, pos: Pos2) -> Option<Side> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let left = (pos.x - rect.left()) / rect.width();
    let right = (rect.right() - pos.x) / rect.width();
    let top = (pos.y - rect.top()) / rect.height();
    let bottom = (rect.bottom() - pos.y) / rect.height();
    let closest = left.min(right).min(top).min(bottom);
    if closest >= EDGE_ZONE {
        return None;
    }
    if closest == left {
        return Some(Side::Left);
    }
    if closest == right {
        return Some(Side::Right);
    }
    if closest == top {
        return Some(Side::Above);
    }
    Some(Side::Below)
}

fn side_rect(rect: Rect, side: Side) -> Rect {
    let half = vec2(rect.width() / 2.0, rect.height() / 2.0);
    match side {
        Side::Left => Rect::from_min_size(rect.min, vec2(half.x, rect.height())),
        Side::Right => Rect::from_min_size(
            pos2(rect.left() + half.x, rect.top()),
            vec2(half.x, rect.height()),
        ),
        Side::Above => Rect::from_min_size(rect.min, vec2(rect.width(), half.y)),
        Side::Below => Rect::from_min_size(
            pos2(rect.left(), rect.top() + half.y),
            vec2(rect.width(), half.y),
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Grip {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

const GRIPS: [(Grip, bool); 12] = [
    (Grip::Left, false),
    (Grip::Right, false),
    (Grip::Top, false),
    (Grip::Bottom, false),
    (Grip::TopLeft, false),
    (Grip::TopLeft, true),
    (Grip::TopRight, false),
    (Grip::TopRight, true),
    (Grip::BottomLeft, false),
    (Grip::BottomLeft, true),
    (Grip::BottomRight, false),
    (Grip::BottomRight, true),
];

impl Grip {
    fn cursor(self) -> CursorIcon {
        match self {
            Grip::Left | Grip::Right => CursorIcon::ResizeHorizontal,
            Grip::Top | Grip::Bottom => CursorIcon::ResizeVertical,
            Grip::TopLeft | Grip::BottomRight => CursorIcon::ResizeNwSe,
            Grip::TopRight | Grip::BottomLeft => CursorIcon::ResizeNeSw,
        }
    }

    fn rect(self, upright: bool, size: Vec2) -> Rect {
        let (reach, corner) = (HANDLE_REACH, GRIP_CORNER);
        let far = pos2(size.x - reach, size.y - reach);
        let along = vec2(size.x - corner * 2.0, reach);
        let down = vec2(reach, size.y - corner * 2.0);
        let arm = match upright {
            true => vec2(reach, corner),
            false => vec2(corner, reach),
        };
        let right = size.x - arm.x;
        let bottom = size.y - arm.y;
        match self {
            Grip::Left => Rect::from_min_size(pos2(0.0, corner), down),
            Grip::Right => Rect::from_min_size(pos2(far.x, corner), down),
            Grip::Top => Rect::from_min_size(pos2(corner, 0.0), along),
            Grip::Bottom => Rect::from_min_size(pos2(corner, far.y), along),
            Grip::TopLeft => Rect::from_min_size(Pos2::ZERO, arm),
            Grip::TopRight => Rect::from_min_size(pos2(right, 0.0), arm),
            Grip::BottomLeft => Rect::from_min_size(pos2(0.0, bottom), arm),
            Grip::BottomRight => Rect::from_min_size(pos2(right, bottom), arm),
        }
    }

    fn edge(self) -> WindowEdge {
        match self {
            Grip::Left => WindowEdge::Left,
            Grip::Right => WindowEdge::Right,
            Grip::Top => WindowEdge::Top,
            Grip::Bottom => WindowEdge::Bottom,
            Grip::TopLeft => WindowEdge::TopLeft,
            Grip::TopRight => WindowEdge::TopRight,
            Grip::BottomLeft => WindowEdge::BottomLeft,
            Grip::BottomRight => WindowEdge::BottomRight,
        }
    }

    fn resized(self, start: Rect, delta: Vec2) -> Rect {
        let mut min = start.min;
        let mut max = start.max;
        if matches!(self, Grip::Left | Grip::TopLeft | Grip::BottomLeft) {
            min.x = (min.x + delta.x).min(max.x - MIN_WINDOW_SIZE.x);
        }
        if matches!(self, Grip::Right | Grip::TopRight | Grip::BottomRight) {
            max.x = (max.x + delta.x).max(min.x + MIN_WINDOW_SIZE.x);
        }
        if matches!(self, Grip::Top | Grip::TopLeft | Grip::TopRight) {
            min.y = (min.y + delta.y).min(max.y - MIN_WINDOW_SIZE.y);
        }
        if matches!(self, Grip::Bottom | Grip::BottomLeft | Grip::BottomRight) {
            max.y = (max.y + delta.y).max(min.y + MIN_WINDOW_SIZE.y);
        }
        Rect::from_min_max(min, max)
    }
}

fn vacant_leaf(state: &DockState) -> LeafId {
    state
        .focused_leaf()
        .unwrap_or_else(|| state.leaves(state.main())[0])
}

pub(crate) struct DockConfig {
    pub(crate) api: ComponentRef<DockApi>,
    pub(crate) state: Binding<DockState>,
    pub(crate) on_close: Callback<TabId>,
    pub(crate) title: Func<TabId, String>,
    pub(crate) group_title: Func<GroupId, Option<String>>,
    pub(crate) icon: Func<TabId, Option<Glyph>>,
    pub(crate) picture: Func<TabId, Option<Image>>,
    pub(crate) closable: Func<TabId, bool>,
    pub(crate) menu: MenuStyle,
    pub(crate) stack_below: f32,
    pub(crate) mode: Signal<DockMode>,
    pub(crate) drag_modifier: Prop<Option<Modifiers>>,
    pub(crate) splitter_thickness: f32,
    pub(crate) splitter_reach: f32,
    pub(crate) group_inset: f32,
    pub(crate) icon_sidebar_width: f32,
    pub(crate) inset: Prop<f32>,
    pub(crate) tab: RenderFn<DockTabHandle>,
    pub(crate) content: RenderFn<TabId>,
    pub(crate) empty: RenderFn<LeafId>,
    pub(crate) fullscreen: Option<RenderFn<NodeId>>,
    pub(crate) away: Option<RenderFn<DockAwayHandle>>,
    pub(crate) chrome: Option<RenderFn<DockChromeHandle>>,
    pub(crate) bar: Option<RenderFn<DockBarHandle>>,
    pub(crate) splitter: Option<RenderFn<DockSplitterHandle>>,
    pub(crate) grip: Option<RenderFn<DockGripHandle>>,
    pub(crate) highlight: Option<RenderFn<()>>,
    pub(crate) preview: Option<RenderFn<DockPreviewHandle>>,
    pub(crate) stack: Option<RenderFn<DockStackHandle>>,
    pub(crate) switcher: Option<RenderFn<DockSwitcherHandle>>,
    pub(crate) switch: Option<RenderFn<DockSwitchHandle>>,
    pub(crate) dialogs: Memo<Vec<(TabId, TabId)>>,
    pub(crate) dialog: Func<TabId, Option<DialogParts>>,
    pub(crate) dialog_chrome: Option<RenderFn<DockDialogHandle>>,
    pub(crate) dialog_sheet: Option<RenderFn<DockSheetHandle>>,
    pub(crate) dialog_scrim: Memo<Color32>,
    pub(crate) dialog_margin: u16,
    pub(crate) dialog_gap: u16,
}

#[component]
pub(crate) fn Dock(config: DockConfig) -> NodeId {
    let DockConfig {
        api,
        state,
        on_close,
        title,
        group_title,
        icon,
        picture,
        closable,
        menu,
        stack_below,
        mode: published,
        drag_modifier,
        splitter_thickness,
        splitter_reach,
        group_inset,
        icon_sidebar_width,
        inset,
        tab,
        content,
        empty,
        fullscreen,
        away,
        chrome,
        bar,
        splitter,
        grip,
        highlight,
        preview,
        stack,
        switcher,
        switch,
        dialogs,
        dialog,
        dialog_chrome,
        dialog_sheet,
        dialog_scrim,
        dialog_margin,
        dialog_gap,
    } = config;
    view! {
        <PlacementBuilder>|dock_rect: ReadSignal<Rect>| {
            let mode = create_memo(move || {
                let width = dock_rect.get().width();
                match width > 0.0 && width < stack_below {
                    true => DockMode::Stacked,
                    false => DockMode::Tiled,
                }
            });
            create_effect(move || published.set(mode.get()));
            let dialog_focus = create_signal(None);
            let attention = create_signal((0, None));
            let given = create_signal((0, None));
            let menus = create_signal(DockMenus::new());
            let offers = create_signal(HashMap::new());
            let drag = create_signal(None);
            let dock = State {
                state,
                windows: store(RefCell::default()),
                fullscreen_layer: NodeRef::new(),
                panels: store(RefCell::default()),
                owner: store(owner_scope()),
                on_close,
                drag,
                title,
                group_title,
                icon,
                picture,
                closable,
                menu,
                drag_modifier: create_memo(move || drag_modifier.get()),
                menus,
                offers,
                stack,
                switcher,
                switch,
                mode,
                dialogs,
                dialog,
                dialog_chrome: dialog_chrome
                    .unwrap_or_else(|| RenderFn::new(|handle: DockDialogHandle| handle.body)),
                dialog_sheet,
                dialog_scrim,
                dialog_margin,
                dialog_gap,
                dialog_frames: store(RefCell::default()),
                dialog_bodies: store(RefCell::default()),
                dialog_focus,
                attention,
                given,
                remembered: store(RefCell::default()),
                thickness: splitter_thickness,
                reach: splitter_reach,
                group_inset,
                icon_sidebar_width,
                rect: dock_rect,
                opens: opens_windows(),
                home: use_window(),
                placements: create_signal(HashMap::new()),
                panes: store(RefCell::default()),
                bars: store(RefCell::default()),
                tab,
                content,
                empty,
                fullscreen: fullscreen.unwrap_or_else(|| {
                    RenderFn::new(|body: NodeId| {
                        view! {
                            <Frame>{body}</Frame>
                        }
                    })
                }),
                away: away.unwrap_or_else(|| {
                    RenderFn::new(|_| {
                        view! {
                            <Frame />
                        }
                    })
                }),
                chrome: chrome.unwrap_or_else(|| RenderFn::new(|handle: DockChromeHandle| handle.content)),
                bar: bar.unwrap_or_else(|| {
                    RenderFn::new(|handle| {
                        view! {
                            <PlainBar handle />
                        }
                    })
                }),
                splitter: splitter.unwrap_or_else(|| {
                    RenderFn::new(|_| {
                        view! {
                            <Frame />
                        }
                    })
                }),
                grip: grip.unwrap_or_else(|| {
                    RenderFn::new(|_| {
                        view! {
                            <Frame />
                        }
                    })
                }),
                highlight: highlight.unwrap_or_else(|| {
                    RenderFn::new(|()| {
                        view! {
                            <Frame />
                        }
                    })
                }),
                preview: preview.unwrap_or_else(|| {
                    RenderFn::new(|_| {
                        view! {
                            <Frame />
                        }
                    })
                }),
            };
            api.fill(DockApi(dock));
            on_shortcut(move |press: KeyPress| {
                if !press.pressed || press.key != Key::Tab || !press.modifiers.ctrl || press.modifiers.alt
                {
                    return false;
                }
                dock.cycle(press.modifiers.shift)
            });
            create_effect(move || {
                let tabs = state.with(DockState::all_tabs);
                dock.keep_panels(&tabs);
            });
            on_cleanup(move || dock.keep_panels(&[]));
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) padding=inset>
                        <List spacing=0.0>
                            <Dynamic value={mode}>|mode: DockMode| {
                                match mode {
                                    DockMode::Tiled => view! {
                                        <DockTiles dock @sizing=ItemSize::Percent(100.0) />
                                    },
                                    DockMode::Stacked => view! {
                                        <DockStack dock @sizing=ItemSize::Percent(100.0) />
                                    },
                                }
                            }</Dynamic>
                        </List>
                    </Frame>
                    <DockFullscreenView dock={dock} />
                    <DockDialogs dock={dock} />
                    <DockSwitchView dock={dock} />
                </List>
            }
        }</PlacementBuilder>
    }
}

#[component]
fn DockFullscreenView(dock: State) -> NodeId {
    let state = dock.state;
    let fullscreen = create_memo(move || state.with(DockState::fullscreen));
    let panel = create_signal(None);
    create_effect(move || {
        panel.set(fullscreen.get().map(|shown| dock.panel(shown.tab)));
    });
    let open = create_memo(move || fullscreen.get().is_some());
    let area = create_memo(move || fullscreen.get().and_then(|shown| shown.area));
    let placement = create_memo(move || match area.get() {
        Some(_) => Placement::At,
        None => Placement::FillScreen,
    });
    let anchor =
        create_memo(move || OverlayAnchor::Point(area.get().map_or(Pos2::ZERO, |area| area.min)));
    let width = create_memo(move || area.get().map(|area| area.width()));
    let height = create_memo(move || area.get().map(|area| area.height()));
    let front = create_memo(move || dock.fullscreen_in_front());
    let layer = dock.fullscreen_layer;
    create_effect(move || {
        if !front.get() {
            return;
        }
        if let Some(layer) = layer.try_get() {
            untrack(|| layer.raise_overlay());
        }
    });
    let body = move || {
        dock.fullscreen.call(view! {
            <Portal node={panel} />
        })
    };
    view! {
        <Overlay
            @node_ref={&dock.fullscreen_layer}
            anchor
            placement
            policy=LayerPolicy::Pass
            traps_focus=false
            open
        >
            <Interactive
                on_press={move |_: PointerPress| {
                    if let Some(tab) = dock.fullscreen_tab() {
                        dock.show(tab);
                        dock.release_dialog_focus();
                    }
                }}
            >
                <Frame @test_id="dock.fullscreen" width=width height=height>{body()}</Frame>
            </Interactive>
        </Overlay>
    }
}

#[component]
fn DockStack(dock: State) -> NodeId {
    let state = dock.state;
    let shown = create_memo(move || state.with(DockState::stacked_tab));
    let away = create_memo(move || {
        state.with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab))
    });
    let panel = create_signal(None);
    let previous = Cell::new(None);
    create_effect(move || {
        let tab = shown.get().filter(|tab| away.get() != Some(*tab));
        let left = previous.replace(tab).filter(|left| Some(*left) != tab);
        let held = untrack(|| left.is_some_and(|left| dock.remember_focus(left)));
        panel.set(tab.map(|tab| dock.panel(tab)));
        if let Some(tab) = tab.filter(|_| held) {
            untrack(|| dock.give_keyboard(tab));
        }
    });
    let vacant = create_memo(move || shown.get().is_none());
    let occupied = create_memo(move || shown.get().is_some());
    let backward = create_memo(move || state.with(|state| state.back().is_some()));
    let switching = create_signal(false);
    let tabs = create_memo(move || state.with(DockState::recent_tabs));
    let bar = dock
        .stack
        .map(|stack| stack.call(stack_handle(dock, shown, backward, tabs, switching)));
    let switcher = dock
        .switcher
        .map(|switcher| switcher.call(switcher_handle(dock, shown, tabs, switching)));
    let barred = create_memo(move || bar.is_some() && occupied.get());
    let gone = dock.away_from(shown);
    let left = create_memo(move || gone.get().is_some());
    let here = create_memo(move || occupied.get() && gone.get().is_none());
    let empty = dock.empty;
    view! {
        <BackSlide
            enabled={backward}
            behind={move || {
                let dock = dock;
                view! {
                    <BackPreview dock switching />
                }
            }}
            on_back={move || dock.go_back()}
        >
            <List spacing=0.0>
                <Show condition={barred}>
                    <Portal node={bar} />
                </Show>
                <Show condition={vacant}>
                    {empty.call(state.with_untracked(vacant_leaf))} @sizing=ItemSize::Percent(100.0)
                </Show>
                <Show condition={here}>
                    <Portal node={panel} @sizing=ItemSize::Percent(100.0) />
                </Show>
                <Show condition={left}>
                    <DockAway dock gone @sizing=ItemSize::Percent(100.0) />
                </Show>
                {switcher}
            </List>
        </BackSlide>
    }
}

#[component]
fn BackPreview(dock: State, switching: Signal<bool>) -> NodeId {
    let Some(visit) = dock.state.with_untracked(DockState::back) else {
        return view! {
            <Frame />
        };
    };
    let closing = untrack(|| dock.closes_going_back(visit));
    let mut after = dock.state.get_untracked();
    match closing {
        true => {
            after.close(visit.to);
        }
        false => {
            after.go_back();
        }
    }
    let behind = visit.from;
    let shown = create_memo(move || Some(behind));
    let backward = after.back().is_some();
    let backward = create_memo(move || backward);
    let tabs = after.recent_tabs();
    let tabs = create_memo(move || tabs.clone());
    let bar = dock
        .stack
        .map(|stack| stack.call(stack_handle(dock, shown, backward, tabs, switching)));
    let panel = dock.panel(behind);
    view! {
        <List spacing=0.0>
            {bar}
            <Portal node={Some(panel)} @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

fn stack_handle(
    dock: State,
    shown: Memo<Option<TabId>>,
    backward: Memo<bool>,
    tabs: Memo<Vec<TabId>>,
    switching: Signal<bool>,
) -> DockStackHandle {
    let title = create_memo(move || shown.get().map(|tab| dock.title(tab)).unwrap_or_default());
    let icon = create_memo(move || shown.get().map(|tab| dock.icon(tab)).unwrap_or_default());
    let menus = dock.menus;
    let menu = create_memo(move || dock_menu_items(menus.read_only(), shown.get()));
    let closable = dock.closable;
    let closable_tabs = create_memo(move || {
        tabs.with(|tabs| tabs.iter().filter(|tab| closable.call(**tab)).count())
    });
    let switch_label = create_memo(move || match closable_tabs.get() {
        1 => "1 open tab".to_owned(),
        count => format!("{count} open tabs"),
    });
    DockStackHandle {
        title,
        icon,
        backward,
        menu,
        back: ClickCallback::new(move || dock.go_back()),
        closable_tabs,
        switch_label,
        open_switcher: ClickCallback::new(move || switching.set(true)),
    }
}

fn switcher_handle(
    dock: State,
    shown: Memo<Option<TabId>>,
    tabs: Memo<Vec<TabId>>,
    switching: Signal<bool>,
) -> DockSwitcherHandle {
    let closable = dock.closable;
    let state = dock.state;
    let open = create_memo(move || {
        tabs.with(|tabs| {
            tabs.iter()
                .copied()
                .filter(|tab| closable.call(*tab))
                .collect::<Vec<_>>()
        })
    });
    let fixed = create_memo(move || {
        state.with(|state| {
            state
                .all_tabs()
                .into_iter()
                .filter(|tab| !closable.call(*tab))
                .collect::<Vec<_>>()
        })
    });
    DockSwitcherHandle {
        open: switching.read_only(),
        close: ClickCallback::new(move || switching.set(false)),
        tabs: open,
        fixed,
        card: Func::new(move |tab: TabId| DockSwitcherCardHandle {
            tab,
            title: create_memo(move || dock.title(tab)),
            icon: create_memo(move || dock.icon(tab)),
            current: create_memo(move || shown.get() == Some(tab)),
            closable: dock.closable.call(tab),
            menu: create_memo(move || {
                closing_switcher(
                    dock_menu_items(dock.menus.read_only(), Some(tab)),
                    switching,
                )
            }),
            pick: ClickCallback::new(move || {
                switching.set(false);
                dock.show(tab);
            }),
            close: ClickCallback::new(move || dock.close_tab(tab)),
        }),
    }
}

fn closing_switcher(nodes: Vec<MenuNode>, switching: Signal<bool>) -> Vec<MenuNode> {
    nodes
        .into_iter()
        .map(|node| match node {
            MenuNode::Action { action, label } => {
                let run = action.clone();
                let mut closing = Action::new(action.id().to_owned(), action.label(), move || {
                    switching.set(false);
                    run.run();
                })
                .glyph(action.glyph())
                .enabled(action.enabled());
                closing = match (action.checked(), action.is_radio()) {
                    (None, _) => closing,
                    (Some(checked), false) => closing.checked(checked),
                    (Some(checked), true) => closing.radio(checked),
                };
                for chord in action.shortcuts() {
                    closing = closing.shortcut(*chord);
                }
                MenuNode::Action {
                    action: closing.detached(),
                    label,
                }
            }
            MenuNode::Submenu(mut submenu) => {
                submenu.items = closing_switcher(std::mem::take(&mut submenu.items), switching);
                MenuNode::Submenu(submenu)
            }
            MenuNode::Separator => MenuNode::Separator,
        })
        .collect()
}

#[component]
fn DockTiles(dock: State) -> NodeId {
    let current = dock.state;
    let main = create_memo(move || current.with(DockState::main));
    let windows = create_memo(move || current.with(DockState::windows));
    view! {
        <DropTarget
            on_over={move |point: Option<(DockDragged, DragPoint)>| dock.drag_over(point.map(|(_, point)| point))}
            on_drop={move |(dragged, point): (DockDragged, DragPoint)| dock.drop_at(dragged, point)}
        >|_: DropHandle|
            <List spacing=0.0>
                <Dynamic value={main}>|surface: SurfaceId| {
                    let dock = dock;
                    view! {
                        <DockTreeView
                            dock
                            tree={Tree::Surface(surface)}
                            @sizing=ItemSize::Percent(100.0)
                        />
                    }
                }</Dynamic>
                <ForEach keys={windows}>|surface: SurfaceId| {
                    let dock = dock;
                    view! {
                        <DockWindowView dock surface />
                    }
                }</ForEach>
                <DockDragLayer dock />
            </List>
        </DropTarget>
    }
}

#[component]
fn PlainBar(handle: DockBarHandle) -> NodeId {
    let DockBarHandle {
        vertical,
        grip,
        tabs,
        ..
    } = handle;
    let direction = match vertical {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    };
    view! {
        <List direction spacing=0.0>{grip}{tabs}</List>
    }
}

struct ChromeParts {
    floating: bool,
    vertical: bool,
    focused: Memo<bool>,
    sidebar_width: Memo<f32>,
    sidebar_splitter: Option<NodeId>,
    bar: Option<DockBarHandle>,
    leaf: Option<LeafId>,
    body: NodeId,
}

#[component]
fn DockChrome(dock: State, parts: ChromeParts) -> NodeId {
    let ChromeParts {
        floating,
        vertical,
        focused,
        sidebar_width,
        sidebar_splitter,
        bar,
        leaf,
        body,
    } = parts;
    let barred = bar.is_some();
    let bar = bar.map(|handle| {
        let face = dock.bar.call(handle);
        view! {
            <Interactive
                press_focus=false
                claims_touch=false
                on_press={move |press: PointerPress| {
                    if let Some(leaf) = leaf {
                        dock.chrome_pressed(leaf, Some(press.pos));
                    }
                }}
                children={face}
            />
        }
    });
    let direction = match vertical {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    };
    let content = view! {
        <List direction spacing=0.0>
            {bar} @sizing={sidebar_size(vertical, sidebar_width)}
            {sidebar_splitter} @sizing=ItemSize::Fixed(dock.thickness + 2.0 * dock.reach)
            {body} @sizing=ItemSize::Percent(100.0)
        </List>
    };
    dock.chrome.call(DockChromeHandle {
        floating,
        barred,
        focused,
        content,
    })
}

#[component]
fn DockTreeView(
    dock: State,
    tree: Tree,
    #[prop(default = None)] hoisted: Prop<Option<LeafId>>,
) -> NodeId {
    let canvas = NodeRef::new();
    dock.panes.value().borrow_mut().insert(tree, canvas);
    on_cleanup(move || {
        dock.panes.try_with(|panes| {
            let mut panes = panes.borrow_mut();
            if panes
                .get(&tree)
                .is_some_and(|current| current.try_get() == canvas.try_get())
            {
                panes.remove(&tree);
            }
        });
    });
    view! {
        <LayoutBuilder>|size: ReadSignal<Vec2>|
            <PlacementBuilder>|placement: ReadSignal<Rect>| {
                let thickness = dock.thickness;
                let state = dock.state;
                let layout = create_memo(move || {
                    let area = Rect::from_min_size(Pos2::ZERO, size.get());
                    state.with(|state| layout_tree(state, tree, area, thickness))
                });
                let leaves = create_memo(move || {
                    layout.with(|layout| layout.leaves.iter().map(|(leaf, _)| *leaf).collect::<Vec<_>>())
                });
                let splits = create_memo(move || {
                    layout.with(|layout| layout.splitters.iter().map(|splitter| splitter.id).collect::<Vec<_>>())
                });
                let hoisted = hoisted.get_untracked();
                let marked_surface = match tree {
                    Tree::Surface(surface) => state
                        .with_untracked(|state| state.window_rect(surface).is_none())
                        .then_some(surface),
                    Tree::Group(_) => None,
                };
                let pane_origin = create_memo(move || placement.get().min);
                view! {
                    <Canvas @node_ref=&canvas>
                        <ForEach keys={leaves}>|leaf: LeafId| {
                            let dock = dock;
                            let layout = layout;
                            let rect = create_memo(move || {
                                layout.with(|layout| layout.leaf_rect(leaf)).unwrap_or(Rect::ZERO)
                            });
                            let x = create_memo(move || rect.get().min.x);
                            let y = create_memo(move || rect.get().min.y);
                            let width = create_memo(move || rect.get().width());
                            let height = create_memo(move || rect.get().height());
                            let hoisted = hoisted == Some(leaf);
                            view! {
                                <CanvasItem x={x} y={y} width={width} height={height}>
                                    <DockPanelView dock tree leaf hoisted />
                                </CanvasItem>
                            }
                        }</ForEach>
                        <ForEach keys={splits}>|split: SplitId| {
                            let dock = dock;
                            let layout = layout;
                            let reach = dock.reach;
                            let rect = create_memo(move || {
                                layout.with(|layout| layout.splitter(split)).map_or(Rect::ZERO, |splitter| {
                                    let outset = match splitter.direction {
                                        Direction::Horizontal => vec2(reach, 0.0),
                                        Direction::Vertical => vec2(0.0, reach),
                                    };
                                    splitter.handle.expand2(outset)
                                })
                            });
                            let x = create_memo(move || rect.get().min.x);
                            let y = create_memo(move || rect.get().min.y);
                            let width = create_memo(move || rect.get().width());
                            let height = create_memo(move || rect.get().height());
                            view! {
                                <CanvasItem x={x} y={y} width={width} height={height}>
                                    <DockSplitterView dock tree split />
                                </CanvasItem>
                            }
                        }</ForEach>
                        <Show condition={marked_surface.is_some()}>
                            <DockDropMarker
                                dock={dock}
                                surface={marked_surface.unwrap_or_else(|| unreachable!())}
                                origin={pane_origin}
                            />
                        </Show>
                    </Canvas>
                }
            }</PlacementBuilder>
        </LayoutBuilder>
    }
}

#[component]
fn DockDropMarker(
    dock: State,
    surface: SurfaceId,
    origin: Memo<Pos2>,
) -> DynamicSegment<CanvasItem> {
    let marker = create_memo(move || {
        let state = dock.state.get_untracked();
        let drag = dock.drag.get()?;
        let leaf = drag.target?.leaf()?;
        if state.surface_of(leaf) != Some(surface) {
            return None;
        }
        let highlight = drag.highlight?;
        Some(Rect::from_min_size(
            highlight.min - origin.get().to_vec2(),
            highlight.size(),
        ))
    });
    let shown = create_memo(move || marker.get().is_some());
    let rect = create_memo(move || marker.get().unwrap_or(Rect::ZERO));
    let x = create_memo(move || rect.get().min.x);
    let y = create_memo(move || rect.get().min.y);
    let width = create_memo(move || rect.get().width());
    let height = create_memo(move || rect.get().height());
    view! {
        <Show condition={shown}>
            <CanvasItem x={x} y={y} width={width} height={height}>
                {dock.highlight.call(())}
            </CanvasItem>
        </Show>
    }
}

#[component]
fn DockPanelView(dock: State, tree: Tree, leaf: LeafId, hoisted: bool) -> NodeId {
    let state = dock.state;
    let focused = create_memo(move || state.with(|state| state.focused_leaf() == Some(leaf)));
    let surface = state
        .with_untracked(|state| state.surface_of(leaf))
        .unwrap_or_else(|| state.with_untracked(DockState::main));
    let nested = matches!(tree, Tree::Group(_));
    let floating = !nested && state.with_untracked(|state| state.window_rect(surface).is_some());
    let vertical = create_memo(move || !hoisted && state.with(|state| state.is_vertical(leaf)));
    let contents = create_memo(move || {
        state.with(|state| {
            state
                .entries(leaf)
                .into_iter()
                .flat_map(|entry| state.entry_tabs(entry))
                .collect::<Vec<_>>()
        })
    });
    let vacant = create_memo(move || contents.with(Vec::is_empty));
    let sidebar_width = dock.sidebar_width(Some(leaf));
    let icons_only = dock.icons_only(Some(leaf));
    let close = ClickCallback::new(move || dock.close_leaf(leaf));
    let menu = dock.leaf_menu(Some(leaf));
    let closable = dock.all_closable(contents);
    let claimed = dock.drag_modifier;
    let resized = modifier_resize(dock, Resized::Docked);
    view! {
        <Interactive
            claims_touch=false
            claim_modifiers={claimed}
            on_secondary_drag={resized}
            on_press={move |press: PointerPress| {
                if dock.modifier_held(press.modifiers) {
                    return;
                }
                let hosted = dock.state.with_untracked(|state| state.active_entry(leaf));
                let inside = hosted
                    .and_then(Entry::group)
                    .and_then(|group| dock.pane_rect(Tree::Group(group)))
                    .is_some_and(|inner| inner.contains(press.pos));
                if !inside {
                    dock.edit(|state| state.focus(leaf));
                    dock.release_dialog_focus();
                }
            }}
        >
            <List spacing=0.0>
                <Dynamic value={vertical}>|vertical: bool| {
                    let dock = dock;
                    let bar = (!hoisted).then(|| DockBarHandle {
                        floating,
                        vertical,
                        icons_only,
                        focused,
                        grip: view! {
                            <DockPaneGrip
                                dock={dock}
                                leaf
                                vertical
                                focused={focused}
                                pane={GripPane { menu, closable, close }}
                            />
                        },
                        tabs: Some(view! {
                            <DockTabBar dock={dock} leaf vertical />
                        }),
                        title: create_memo(String::new),
                        vacant,
                        closable,
                        menu,
                        close,
                    });
                    let parts = ChromeParts {
                        floating,
                        vertical,
                        focused,
                        sidebar_width,
                        sidebar_splitter: (vertical && !hoisted).then(|| view! {
                            <DockSidebarSplitter dock={dock} leaf />
                        }),
                        bar,
                        leaf: Some(leaf),
                        body: view! {
                            <DockPanelBody dock={dock} leaf />
                        },
                    };
                    view! {
                        <DockChrome dock parts @sizing=ItemSize::Percent(100.0) />
                    }
                }</Dynamic>
            </List>
        </Interactive>
    }
}

#[component]
fn DockPaneGrip(
    dock: State,
    leaf: LeafId,
    vertical: bool,
    focused: Memo<bool>,
    pane: GripPane,
) -> NodeId {
    let face = dock.grip.call(DockGripHandle {
        floating: false,
        vertical,
        focused,
    });
    let menu = dock.menu;
    let title = create_memo(move || dock.dragged_title(DockDragged::Pane(leaf)));
    let icon = create_memo(move || dock.dragged_icon(DockDragged::Pane(leaf)));
    let preview = dock.preview;
    view! {
        <Draggable
            @test_id={dock.pane_test_id("dock.grip", leaf)}
            payload={DockDragged::Pane(leaf)}
            cursor=CursorIcon::Grab
            touch_drags=true
            preview={move |dragged: DockDragged| preview.call(DockPreviewHandle {
                dragged,
                title,
                icon,
            })}
            on_drag_change={move |dragging: bool| match dragging {
                true => dock.begin_drag(DockDragged::Pane(leaf)),
                false => dock.end_drag(),
            }}
            on_drop_outside={move |_: DragPoint| dock.drop_outside(DockDragged::Pane(leaf))}
        >|_: DragHandle|
            <GripMenu
                menu
                vertical
                icons_only={dock.icons_only(Some(leaf))}
                pane
                toggle={move || dock.toggle_vertical(leaf)}
                toggle_icons={move || dock.toggle_icons_only(leaf)}
            >
                {face}
            </GripMenu>
        </Draggable>
    }
}

#[component]
fn DockPanelBody(dock: State, leaf: LeafId) -> NodeId {
    let state = dock.state;
    let group =
        create_memo(move || state.with(|state| state.active_entry(leaf).and_then(Entry::group)));
    view! {
        <List spacing=0.0>
            <Dynamic value={group}>|group: Option<GroupId>| {
                let inset = dock.group_inset;
                match group {
                    Some(group) => view! {
                        <Frame padding=inset @sizing=ItemSize::Percent(100.0)>
                            <DockTreeView dock tree={Tree::Group(group)} />
                        </Frame>
                    },
                    None => view! {
                        <DockTabBody dock leaf @sizing=ItemSize::Percent(100.0) />
                    },
                }
            }</Dynamic>
        </List>
    }
}

#[component]
fn DockTabBody(dock: State, leaf: LeafId) -> NodeId {
    let state = dock.state;
    let shown = create_memo(move || state.with(|state| state.active_tab(leaf)));
    let away = create_memo(move || {
        state.with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab))
    });
    let panel = create_signal(None);
    let previous = Cell::new(None);
    create_effect(move || {
        let tab = shown.get().filter(|tab| away.get() != Some(*tab));
        let left = previous.replace(tab).filter(|left| Some(*left) != tab);
        let held = untrack(|| left.is_some_and(|left| dock.remember_focus(left)));
        panel.set(tab.map(|tab| dock.panel(tab)));
        if let Some(tab) = tab.filter(|_| held) {
            untrack(|| dock.give_keyboard(tab));
        }
    });
    let vacant = create_memo(move || shown.get().is_none());
    let gone = dock.away_from(shown);
    let left = create_memo(move || gone.get().is_some());
    let occupied = create_memo(move || shown.get().is_some() && gone.get().is_none());
    let empty = dock.empty;
    view! {
        <List spacing=0.0>
            <Show condition={vacant}>{empty.call(leaf)} @sizing=ItemSize::Percent(100.0)</Show>
            <Show condition={occupied}>
                <Portal node={panel} @sizing=ItemSize::Percent(100.0) />
            </Show>
            <Show condition={left}>
                <DockAway dock gone @sizing=ItemSize::Percent(100.0) />
            </Show>
        </List>
    }
}

#[component]
fn DockTabBar(dock: State, leaf: LeafId, vertical: bool) -> NodeId {
    let bar = TabBar::default();
    let (strip, tab_list) = (bar.strip, bar.tabs);
    dock.bars.value().borrow_mut().insert(leaf, bar);
    on_cleanup(move || {
        dock.bars.try_with(|bars| {
            let mut bars = bars.borrow_mut();
            if bars
                .get(&leaf)
                .is_some_and(|current| current.strip.try_get() == strip.try_get())
            {
                bars.remove(&leaf);
            }
        });
    });
    let direction = match vertical {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    };
    let state = dock.state;
    let entries = create_memo(move || state.with(|state| state.entries(leaf)));
    let selected = create_memo(move || Some(state.with(|state| state.active_index(leaf))));
    view! {
        <Scroll direction @node_ref=&strip>
            <Choice
                @node_ref=&tab_list
                options={view! {
                    <ForEach keys={entries}>|entry: Entry| {
                        let dock = dock;
                        let label = create_memo(move || dock.entry_title(entry));
                        view! {
                            <ChoiceOption label={label} />
                        }
                    }</ForEach>
                }}
                selected={Binding::new(selected, move |index: Option<usize>| {
                    let Some(index) = index else {
                        return;
                    };
                    dock.edit(|state| {
                        state.set_active_index(leaf, index);
                        state.focus(leaf);
                    });
                    dock.release_dialog_focus();
                    let picked = tab_list
                        .try_get()
                        .is_some_and(|tabs| tabs.focus_is_within());
                    let shown = dock.state.with_untracked(|state| state.active_tab(leaf));
                    if let Some(tab) = shown.filter(|_| !picked) {
                        dock.give_keyboard(tab);
                    }
                })}
                kind=ChoiceKind::Tabs
                direction
            >|handle: ChoiceOptionHandle| {
                let dock = dock;
                let entry = dock.state.get_untracked().entries(leaf).get(handle.index).copied();
                match entry {
                    Some(entry) => view! {
                        <DockTabView dock leaf entry handle vertical />
                    },
                    None => view! {
                        <Frame />
                    },
                }
            }</Choice>
        </Scroll>
    }
}

#[component]
fn DockTabView(
    dock: State,
    leaf: LeafId,
    entry: Entry,
    handle: ChoiceOptionHandle,
    vertical: bool,
) -> NodeId {
    let ChoiceOptionHandle {
        index,
        selected,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let state = dock.state;
    let title = create_memo(move || dock.entry_title(entry));
    let icon = create_memo(move || dock.entry_icon(entry));
    let picture = create_memo(move || dock.entry_picture(entry));
    let tabs = create_memo(move || state.with(|state| state.entry_tabs(entry)));
    let has_next = create_memo(move || {
        state.with(|state| {
            let entries = state.entries(leaf);
            entries
                .iter()
                .position(|candidate| *candidate == entry)
                .is_some_and(|index| index + 1 < entries.len())
        })
    });
    let dragged = create_memo(move || {
        dock.drag.with(|drag| {
            drag.as_ref()
                .is_some_and(|drag| drag.dragged == DockDragged::Entry(entry))
        })
    });
    let close = ClickCallback::new(move || dock.close_entry(entry));
    let float = ClickCallback::new(move || dock.float_entry(entry));
    let group = ClickCallback::new(move || {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.group_with_next(leaf, index);
            }
        });
    });
    let split = ClickCallback::new(move || {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.split_with_next(leaf, index);
            }
        });
    });
    let ungroup = ClickCallback::new(move || {
        if let Entry::Group(group) = entry {
            dock.edit(|state| state.ungroup(group));
        }
    });
    let pinned = match entry {
        Entry::Group(group) => dock.state.with_untracked(|state| state.is_pinned(group)),
        Entry::Tab(_) => false,
    };
    let held = create_memo(move || match entry {
        Entry::Tab(tab) => state.with(|state| {
            let pinned = state.is_tab_pinned(tab);
            (pinned || state.at_home(tab)).then_some(pinned)
        }),
        Entry::Group(_) => None,
    });
    let toggle_held = ClickCallback::new(move || {
        if let Entry::Tab(tab) = entry {
            dock.edit(|state| {
                let pinned = state.is_tab_pinned(tab);
                state.set_tab_pinned(tab, !pinned);
            });
        }
    });
    let floating = dock.state.with_untracked(|state| {
        !state.is_nested(leaf)
            && state
                .surface_of(leaf)
                .and_then(|surface| state.window_rect(surface))
                .is_some()
    });
    let closable = create_memo(move || {
        !pinned && tabs.with(|tabs| tabs.iter().all(|tab| dock.closable.call(*tab)))
    });
    let waiting = create_memo(move || tabs.with(|tabs| dock.waiting(tabs)));
    let icons_only = dock.icons_only(Some(leaf));
    let face = dock.tab.call(DockTabHandle {
        entry,
        leaf,
        index,
        floating,
        pinned,
        vertical,
        icons_only,
        title,
        icon,
        picture,
        tabs,
        has_next,
        selected,
        hovered,
        active,
        focused,
        dragged,
        closable,
        waiting,
        close,
    });
    let menu = dock.menu;
    let grouped = matches!(entry, Entry::Group(_));
    let fullscreen = create_memo(move || match entry {
        Entry::Tab(tab) => dock
            .offers_fullscreen(tab)
            .then(|| dock.fullscreen_tab() == Some(tab)),
        Entry::Group(_) => None,
    });
    let toggle_fullscreen = ClickCallback::new(move || {
        if let Entry::Tab(tab) = entry {
            dock.toggle_fullscreen(tab);
        }
    });
    let preview = dock.preview;
    let across = match vertical {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    };
    view! {
        <Draggable
            payload={DockDragged::Entry(entry)}
            touch_drag_axis={Some(across)}
            preview={move |dragged: DockDragged| preview.call(DockPreviewHandle {
                dragged,
                title,
                icon,
            })}
            on_drag_change={move |dragging: bool| match dragging {
                true => dock.begin_drag(DockDragged::Entry(entry)),
                false => dock.end_drag(),
            }}
            on_drop_outside={move |_: DragPoint| dock.drop_outside(DockDragged::Entry(entry))}
        >|_: DragHandle|
            <TabMenu
                menu
                grouped
                floating
                pinned
                has_next
                held
                closable
                fullscreen
                close={close}
                float={float}
                group={group}
                split={split}
                ungroup={ungroup}
                toggle_held={toggle_held}
                toggle_fullscreen={toggle_fullscreen}
            >
                {face}
            </TabMenu>
        </Draggable>
    }
}

#[component]
fn TabMenu(
    menu: MenuStyle,
    grouped: bool,
    floating: bool,
    pinned: bool,
    has_next: Memo<bool>,
    held: Memo<Option<bool>>,
    closable: Memo<bool>,
    fullscreen: Memo<Option<bool>>,
    close: ClickCallback,
    float: ClickCallback,
    group: ClickCallback,
    split: ClickCallback,
    ungroup: ClickCallback,
    toggle_held: ClickCallback,
    toggle_fullscreen: ClickCallback,
    children: Child,
) -> NodeId {
    let offered = create_memo(move || fullscreen.get().is_some());
    let fullscreen_label = create_memo(move || match fullscreen.get() {
        Some(true) => "Leave full screen".to_owned(),
        Some(false) | None => "Full screen".to_owned(),
    });
    let stuck = create_memo(move || floating || held.get() == Some(true));
    let homeless = create_memo(move || held.get().is_none());
    let pin_label = create_memo(move || match held.get() {
        Some(true) => "Unpin from group".to_owned(),
        Some(false) | None => "Pin to group".to_owned(),
    });
    let alone = create_memo(move || !has_next.get());
    let unclosable = create_memo(move || !closable.get());
    let (group_label, split_label, close_label) = match grouped {
        false => ("Group with next tab", "Split with next tab", "Close tab"),
        true => (
            "Add next tab to group",
            "Split next tab into group",
            "Close group",
        ),
    };
    view! {
        <ContextMenu
            menu
            items={view! {
                <MenuItem label="Pop out into a window" disabled={stuck} />
                <MenuItem label={group_label} disabled={alone} />
                <MenuItem label={split_label} disabled={alone} />
                <MenuItem label={close_label} disabled={unclosable} />
                <Show condition={!grouped}>
                    <MenuItem label={pin_label} disabled={homeless} />
                </Show>
                <Show condition={grouped}>
                    <MenuItem label="Ungroup" disabled={pinned} />
                </Show>
                <Show condition={offered}>
                    <MenuItem label={fullscreen_label} />
                </Show>
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => float.call(),
                Some(1) => group.call(),
                Some(2) => split.call(),
                Some(3) => close.call(),
                Some(4) if grouped => ungroup.call(),
                Some(4) => toggle_held.call(),
                Some(5) => toggle_fullscreen.call(),
                _ => {}
            }}
        >
            <Interactive
                on_middle_click={move || {
                    if closable.get_untracked() {
                        close.call();
                    }
                }}
            >
                {children}
            </Interactive>
        </ContextMenu>
    }
}

#[derive(Clone, Copy)]
struct GripPane {
    menu: Memo<Vec<MenuNode>>,
    closable: Memo<bool>,
    close: ClickCallback,
}

#[component]
fn GripMenu(
    menu: MenuStyle,
    vertical: bool,
    icons_only: Memo<bool>,
    pane: GripPane,
    toggle: ClickCallback,
    toggle_icons: ClickCallback,
    children: Child,
) -> NodeId {
    let label = match vertical {
        true => "Show tabs across the top",
        false => "Show tabs in a sidebar",
    };
    let checked = create_memo(move || Some(icons_only.get()));
    let GripPane {
        menu: tab_menu,
        closable,
        close,
    } = pane;
    let unclosable = create_memo(move || !closable.get());
    let tab_menu = create_memo(move || {
        let mut nodes = tab_menu.get();
        if !nodes.is_empty() {
            nodes.insert(0, MenuNode::Separator);
        }
        nodes
    });
    view! {
        <ContextMenu
            menu
            items={view! {
                <MenuItem label />
                <MenuItem label="Icons only" checked />
                <MenuItem label="Close all tabs" disabled={unclosable} />
                <MenuNodes nodes={tab_menu} />
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => toggle.call(),
                Some(1) => toggle_icons.call(),
                Some(2) => close.call(),
                _ => {}
            }}
        >
            {children}
        </ContextMenu>
    }
}

#[component]
fn DockSplitterView(dock: State, tree: Tree, split: SplitId) -> NodeId {
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let state = dock.state;
    let direction = state
        .with_untracked(|state| state.split_direction(split))
        .unwrap_or(Direction::Horizontal);
    let fraction = create_memo(move || state.with(|state| state.split_fraction(split)));
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Splitter);
        node.set_numeric_value(f64::from(fraction.get() * 100.0));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(100.0);
        node
    }));
    let face = dock.splitter.call(DockSplitterHandle {
        direction,
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
    });
    let held: Rc<Cell<Option<(f32, Pos2)>>> = Rc::new(Cell::new(None));
    let grabbed = held.clone();
    let shown = Rc::new(move || {
        dock.splitter_of(tree, split)
            .map_or_else(|| fraction.get_untracked(), |splitter| splitter.fraction)
    });
    let start = shown.clone();
    view! {
        <Interactive
            capture_presses=true
            focusable=true
            on_focus_change={focused}
            on_key={move |press: KeyPress| {
                if !press.pressed {
                    return false;
                }
                let step = match (direction, press.key) {
                    (Direction::Horizontal, Key::ArrowLeft) => -SPLIT_STEP,
                    (Direction::Horizontal, Key::ArrowRight) => SPLIT_STEP,
                    (Direction::Vertical, Key::ArrowUp) => -SPLIT_STEP,
                    (Direction::Vertical, Key::ArrowDown) => SPLIT_STEP,
                    _ => return false,
                };
                let next = shown() + step;
                dock.edit(|state| state.set_split_fraction(split, next));
                true
            }}
            cursor={match direction {
                Direction::Horizontal => CursorIcon::ResizeHorizontal,
                Direction::Vertical => CursorIcon::ResizeVertical,
            }}
            touch_drag_axis={Some(direction)}
            on_hover_change={hovered}
            on_active_change={active}
            on_press={move |press: PointerPress| {
                grabbed.set(Some((start(), press.pos)));
            }}
            on_drag={move |press: PointerPress| {
                let Some((start, from)) = held.get() else {
                    return;
                };
                let moved = direction.main(press.pos - from);
                if moved == 0.0 {
                    return;
                }
                let Some(splitter) = dock.splitter_of(tree, split) else {
                    return;
                };
                let fraction = fraction_moved(
                    splitter.area,
                    splitter.direction,
                    dock.thickness,
                    start,
                    moved,
                );
                dock.edit(|state| state.set_split_fraction(split, fraction));
            }}
        >
            <Frame>{face}</Frame>
        </Interactive>
    }
}

fn fitted(size: Vec2, bounds: Vec2) -> Vec2 {
    match bounds.x > 0.0 && bounds.y > 0.0 {
        true => size.min(bounds),
        false => size,
    }
}

fn reachable_origin(rect: Rect, bounds: Vec2, bar: f32) -> Pos2 {
    let keep = rect.width().min(WINDOW_KEEP_VISIBLE);
    let x = rect.min.x.clamp(0.0, (bounds.x - keep).max(0.0));
    let y = rect.min.y.clamp(0.0, (bounds.y - bar).max(0.0));
    pos2(x, y)
}

#[component]
fn DockSidebarSplitter(dock: State, leaf: LeafId) -> NodeId {
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let state = dock.state;
    let width = dock.sidebar_target(Some(leaf));
    let narrow = dock.icon_sidebar_width;
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Splitter);
        node.set_numeric_value(f64::from(width.get()));
        node.set_min_numeric_value(f64::from(narrow));
        node
    }));
    let face = dock.splitter.call(DockSplitterHandle {
        direction: Direction::Horizontal,
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
    });
    let held: Rc<Cell<Option<(f32, Pos2, f32)>>> = Rc::new(Cell::new(None));
    let grabbed = held.clone();
    view! {
        <Interactive
            @test_id={dock.pane_test_id("dock.sidebar", leaf)}
            focusable=true
            on_focus_change={focused}
            on_key={move |press: KeyPress| {
                if !press.pressed {
                    return false;
                }
                let icons_only = state.with_untracked(|state| state.is_icons_only(leaf));
                let stored = state.with_untracked(|state| state.sidebar_width(leaf));
                match (press.key, icons_only) {
                    (Key::ArrowLeft, true) => {}
                    (Key::ArrowLeft, false) if stored <= MIN_SIDEBAR_WIDTH => {
                        dock.edit(|state| state.set_icons_only(leaf, true));
                    }
                    (Key::ArrowLeft, false) => {
                        dock.edit(|state| state.set_sidebar_width(leaf, stored - SIDEBAR_STEP));
                    }
                    (Key::ArrowRight, true) => {
                        dock.edit(|state| state.set_icons_only(leaf, false));
                    }
                    (Key::ArrowRight, false) => {
                        dock.edit(|state| state.set_sidebar_width(leaf, stored + SIDEBAR_STEP));
                    }
                    _ => return false,
                }
                true
            }}
            cursor=CursorIcon::ResizeHorizontal
            capture_presses=true
            on_hover_change={hovered}
            on_active_change={active}
            on_press={move |press: PointerPress| {
                let start = state.with_untracked(|state| state.sidebar_width(leaf));
                let start = match state.with_untracked(|state| state.is_icons_only(leaf)) {
                    true => narrow,
                    false => start,
                };
                grabbed.set(Some((start, press.pos, press.pos.x)));
            }}
            on_drag={move |press: PointerPress| {
                let Some((start, from, last)) = held.get() else {
                    return;
                };
                let moved = press.pos.x - last;
                if moved == 0.0 {
                    return;
                }
                held.set(Some((start, from, press.pos.x)));
                let next = start + (press.pos.x - from.x);
                dock.edit(|state| state.drag_sidebar(leaf, next, moved));
            }}
            children={face}
        />
    }
}

#[component]
fn DockWindowView(dock: State, surface: SurfaceId) -> NodeId {
    let frame = NodeRef::new();
    dock.windows.value().borrow_mut().insert(surface, frame);
    on_cleanup(move || {
        dock.windows
            .try_with(|windows| windows.borrow_mut().remove(&surface));
    });
    let state = dock.state;
    let hosted = dock.opens;
    let rect = create_memo(move || {
        state
            .with(|state| state.window_rect(surface))
            .unwrap_or(Rect::ZERO)
    });
    let area = dock.rect;
    let overshoot = create_signal(Vec2::ZERO);
    let grip_ref = NodeRef::new();
    let reach = move || grip_reach(&frame, &grip_ref);
    let placed = create_memo(move || {
        if hosted.get() {
            return rect.get();
        }
        let bounds = area.get().size();
        let rect = Rect::from_min_size(rect.get().min, fitted(rect.get().size(), bounds));
        let origin = reachable_origin(rect, bounds, reach());
        Rect::from_min_size(origin, rect.size())
    });
    let outset = vec2(HANDLE_REACH, HANDLE_REACH);
    let origin = create_memo(move || match hosted.get() {
        true => pos2(HANDLE_REACH, HANDLE_REACH),
        false => area.get().min + placed.get().min.to_vec2() + overshoot.get(),
    });
    let band = store(RefCell::new(Band::new(WINDOW_SPRING)));
    let bounce = create_animation(
        move |frame| {
            let band = band.value();
            let mut band = band.borrow_mut();
            let window = placed.get_untracked();
            let bounds = area.get_untracked().size();
            let reach = reach();
            let limit = |origin: Pos2| {
                reachable_origin(Rect::from_min_size(origin, window.size()), bounds, reach)
            };
            let banding = motion().animates();
            let mut origin = window.min;
            if !band.step(frame.now, &mut origin, limit, banding) {
                return false;
            }
            if origin != window.min {
                dock.edit(|state| {
                    state.set_window_rect(surface, Rect::from_min_size(origin, window.size()));
                });
            }
            overshoot.set(band.offset);
            band.moving()
        },
        move || {
            band.value().borrow_mut().settle();
            overshoot.set(Vec2::ZERO);
        },
    );
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    create_effect(move || {
        if !animates.get() {
            band.value().borrow_mut().settle();
            overshoot.set(Vec2::ZERO);
        }
    });
    let at = create_memo(move || origin.get() - outset);
    let width = create_memo(move || placed.get().width());
    let height = create_memo(move || placed.get().height());
    let size = create_memo(move || placed.get().size() + outset * 2.0);
    let outer_width = create_memo(move || size.get().x);
    let outer_height = create_memo(move || size.get().y);
    let focused = create_memo(move || {
        state.with(|state| {
            state
                .focused_leaf()
                .and_then(|leaf| state.surface_of(leaf))
                .is_some_and(|focused| focused == surface)
        })
    });
    let title = create_memo(move || {
        state
            .with(|state| {
                state
                    .leaves(surface)
                    .first()
                    .and_then(|leaf| state.active_entry(*leaf))
            })
            .map(|entry| dock.entry_title(entry))
            .unwrap_or_default()
    });
    let window_title = create_memo(move || Some(title.get()));
    let parent = dock.home();
    let placement = create_memo(move || dock.placement(surface, parent));
    let contents = create_memo(move || state.with(|state| state.surface_tabs(surface)));
    let vacant = create_memo(move || contents.with(Vec::is_empty));
    let close = ClickCallback::new(move || {
        for tab in dock.state.get_untracked().surface_tabs(surface) {
            dock.close_tab(tab);
        }
    });
    let hoisted = state.with_untracked(|state| state.leaves(surface).first().copied());
    let sidebar_width = dock.sidebar_width(hoisted);
    let icons_only = dock.icons_only(hoisted);
    let vertical = create_memo(move || {
        hoisted.is_some_and(|leaf| state.with(|state| state.is_vertical(leaf)))
    });
    let menu = dock.leaf_menu(hoisted);
    let closable = dock.all_closable(contents);
    let overlay = NodeRef::new();
    let shows = dock.state;
    create_effect(move || {
        shows.with(DockState::shows);
        if focused.get() {
            untrack(|| dock.raise_window(overlay));
        }
    });
    let claimed = dock.drag_modifier;
    let view = view! {
        <Window
            @node_ref=&overlay
            size={size}
            at={at}
            placement={placement}
            title={window_title}
            on_report={move |report: WindowReport| dock.window_reported(surface, report)}
        >|handle: WindowHandle| {
            let grabbed = StoredCell::new(None::<(Rect, Pos2)>);
            let resized = Callback::new(modifier_resize(dock, Resized::Window(surface, handle)));
            view! {
                <DropTarget
                    on_over={move |point: Option<(DockDragged, DragPoint)>| dock.drag_over(point.map(|(_, point)| point))}
                    on_drop={move |(dragged, point): (DockDragged, DragPoint)| dock.drop_at(dragged, point)}
                >|_: DropHandle|
                    <Frame width=outer_width height=outer_height>
                        <Canvas>
                            <CanvasItem x=HANDLE_REACH y=HANDLE_REACH width={width} height={height}>
                                <Frame @node_ref=&frame width=width height=height>
                                    <Canvas>
                                        <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                                            <Interactive
                                                capture_at={move |pos: Pos2| {
                                                    dock.surface_rect(surface).is_some_and(|window| window.contains(pos))
                                                        && dock.over_window_bar(surface, pos)
                                                }}
                                                claim_modifiers={claimed}
                                                on_secondary_drag={resized}
                                                on_press={move |press: PointerPress| {
                                                    let carried = dock.modifier_held(press.modifiers);
                                                    let bar = carried || dock.over_window_bar(surface, press.pos);
                                                    let moving = bar && !handle.begin_move();
                                                    if moving { bounce.stop(); }
                                                    grabbed.set(moving.then(|| {
                                                        let window = placed.get_untracked();
                                                        let dimensions = dock.rect.get_untracked().size();
                                                        let stretched = band.value().borrow_mut().grab(dimensions);
                                                        (window.translate(stretched), press.pos)
                                                    }));
                                                    let titled = carried
                                                        || dock
                                                            .pane_rect(Tree::Surface(surface))
                                                            .is_some_and(|pane| !pane.contains(press.pos));
                                                    dock.edit(|state| {
                                                        let inside = state
                                                            .focused_leaf()
                                                            .and_then(|leaf| state.surface_of(leaf))
                                                            == Some(surface);
                                                        if inside && !titled {
                                                            return;
                                                        }
                                                        if let Some(leaf) = state.leaves(surface).first().copied() {
                                                            state.focus(leaf);
                                                        }
                                                    });
                                                    dock.raise_window(overlay);
                                                    dock.release_dialog_focus();
                                                }}
                                                on_drag={move |press: PointerPress| {
                                                    let Some((start, from)) = grabbed.get() else {
                                                        return;
                                                    };
                                                    let placed =
                                                        Rect::from_min_size(start.min + (press.pos - from), start.size());
                                                    let bounds = dock.rect.get_untracked().size();
                                                    let origin = reachable_origin(placed, bounds, reach());
                                                    let banding = motion().animates();
                                                    let band = band.value();
                                                    let mut band = band.borrow_mut();
                                                    band.stretch(origin, placed.min - origin, bounds, banding, beui_core::timer::now());
                                                    overshoot.set(band.offset);
                                                    dock.edit(|state| {
                                                        state.set_window_rect(
                                                            surface,
                                                            Rect::from_min_size(origin, start.size()),
                                                        );
                                                    });
                                                }}
                                                on_active_change={move |active: bool| {
                                                    if active || handle.is_window() {
                                                        return;
                                                    }
                                                    band.value().borrow_mut().release(beui_core::timer::now());
                                                    bounce.start();
                                                }}
                                            >
                                                <List spacing=0.0>
                                                    <Dynamic value={vertical}>|vertical: bool| {
                                                        let dock = dock;
                                                        let grip_face = dock.grip.call(DockGripHandle {
                                                            floating: true,
                                                            vertical,
                                                            focused,
                                                        });
                                                        let grip_face = view! {
                                                            <GripMenu
                                                                menu={dock.menu}
                                                                vertical
                                                                icons_only
                                                                pane={GripPane { menu, closable, close }}
                                                                toggle={move || {
                                                                    if let Some(leaf) = hoisted {
                                                                        dock.toggle_vertical(leaf);
                                                                    }
                                                                }}
                                                                toggle_icons={move || {
                                                                    if let Some(leaf) = hoisted {
                                                                        dock.toggle_icons_only(leaf);
                                                                    }
                                                                }}
                                                            >
                                                                {grip_face}
                                                            </GripMenu>
                                                        };
                                                        let grip = view! {
                                                            <Interactive
                                                                @node_ref=&grip_ref
                                                                cursor=CursorIcon::Grab
                                                                children={grip_face}
                                                            />
                                                        };
                                                        let tabs = hoisted.map(|leaf| {
                                                            view! {
                                                                <DockTabBar
                                                                    dock={dock}
                                                                    leaf
                                                                    vertical
                                                                />
                                                            }
                                                        });
                                                        let pane = view! {
                                                            <DockTreeView
                                                                dock={dock}
                                                                tree={Tree::Surface(surface)}
                                                                hoisted
                                                            />
                                                        };
                                                        let parts = ChromeParts {
                                                            floating: true,
                                                            vertical,
                                                            focused,
                                                            sidebar_width,
                                                            sidebar_splitter: hoisted.filter(|_| vertical).map(|leaf| view! {
                                                                <DockSidebarSplitter
                                                                    dock={dock}
                                                                    leaf
                                                                />
                                                            }),
                                                            bar: Some(DockBarHandle {
                                                                floating: true,
                                                                vertical,
                                                                icons_only,
                                                                focused,
                                                                grip,
                                                                tabs,
                                                                title,
                                                                vacant,
                                                                closable,
                                                                menu,
                                                                close,
                                                            }),
                                                            leaf: hoisted,
                                                            body: pane,
                                                        };
                                                        view! {
                                                            <DockChrome
                                                                dock
                                                                parts
                                                                @sizing=ItemSize::Percent(100.0)
                                                            />
                                                        }
                                                    }</Dynamic>
                                                </List>
                                            </Interactive>
                                        </CanvasItem>
                                        <DockDropMarker dock={dock} surface origin={origin} />
                                    </Canvas>
                                </Frame>
                            </CanvasItem>
                            <ForEach keys={GRIPS.to_vec()}>|(grip, upright): (Grip, bool)| {
                                let dock = dock;
                                let handle_rect = create_memo(move || grip.rect(upright, size.get()));
                                let x = create_memo(move || handle_rect.get().min.x);
                                let y = create_memo(move || handle_rect.get().min.y);
                                let width = create_memo(move || handle_rect.get().width());
                                let height = create_memo(move || handle_rect.get().height());
                                let held: Rc<Cell<Option<(Rect, Pos2)>>> = Rc::new(Cell::new(None));
                                let pressed = held.clone();
                                view! {
                                    <CanvasItem x={x} y={y} width={width} height={height}>
                                        <Interactive
                                            cursor={grip.cursor()}
                                            capture_presses=true
                                            on_press={move |press: PointerPress| {
                                                let by_host = handle.begin_resize(grip.edge());
                                                pressed.set((!by_host).then(|| (placed.get_untracked(), press.pos)));
                                                let leaf = dock.state.with_untracked(|state| {
                                                    state
                                                        .focused_leaf()
                                                        .filter(|leaf| state.surface_of(*leaf) == Some(surface))
                                                        .or_else(|| state.leaves(surface).first().copied())
                                                });
                                                if let Some(leaf) = leaf {
                                                    dock.edit(|state| state.focus(leaf));
                                                }
                                                dock.raise_window(overlay);
                                                dock.release_dialog_focus();
                                                if let Some(leaf) = leaf {
                                                    dock.chrome_pressed(leaf, None);
                                                }
                                            }}
                                            on_drag={move |press: PointerPress| {
                                                let Some((start, from)) = held.get() else {
                                                    return;
                                                };
                                                let resized = grip.resized(start, press.pos - from);
                                                dock.edit(|state| state.set_window_rect(surface, resized));
                                            }}
                                        ></Interactive>
                                    </CanvasItem>
                                }
                            }</ForEach>
                        </Canvas>
                    </Frame>
                </DropTarget>
            }
        }</Window>
    };
    create_effect(move || untrack(|| dock.raise_window(overlay)));
    view
}

fn grip_reach(frame: &NodeRef, grip: &NodeRef) -> f32 {
    let (Some(frame), Some(grip)) = (frame.try_get(), grip.try_get()) else {
        return 0.0;
    };
    let placements = placements();
    let reach = || Some(placements.rect(grip)?.bottom() - placements.rect(frame)?.top());
    reach().unwrap_or(0.0)
}

#[component]
fn DockDragLayer(dock: State) -> NodeId {
    let drag = dock.drag;
    let marked = create_memo(move || {
        drag.with(|drag| {
            let drag = drag.as_ref()?;
            matches!(drag.target?, DockDrop::Window { .. })
                .then_some(drag.highlight)
                .flatten()
        })
    });
    let marking = create_memo(move || marked.get().is_some());
    let highlight = create_memo(move || marked.get().unwrap_or(Rect::ZERO));
    let marker_anchor = create_memo(move || highlight.get().min)
        .into_prop()
        .map(OverlayAnchor::Point);
    let marker_width = create_memo(move || highlight.get().width());
    let marker_height = create_memo(move || highlight.get().height());
    view! {
        <List spacing=0.0>
            <Overlay
                anchor={marker_anchor}
                placement=Placement::BelowStart
                policy=LayerPolicy::Inert
                traps_focus=false
                open={marking}
            >
                <Frame width=marker_width height=marker_height>{dock.highlight.call(())}</Frame>
            </Overlay>
        </List>
    }
}

#[derive(Clone, Copy)]
pub struct DockApi(State);

impl DockApi {
    pub fn state(&self) -> DockState {
        self.0.state.get()
    }
}
