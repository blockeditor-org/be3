use beui_view::types::icons::Glyph;
mod bars;
mod compact;
mod dialog;
mod fluid;
mod fullscreen;
mod hosting;
mod keyboard;
mod modifier_drag;
mod reveal;
mod switch;
#[cfg(test)]
mod tests;

use be_lib_stable_hash::HashMap;
use beui_view::types::{AboveEmbeds, LayerPolicy};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use accesskit::{Node, Role};

use beui_components_unstyled::Choice;
use beui_components_unstyled::ChoiceItem;
use beui_components_unstyled::ChoiceKind;
use beui_components_unstyled::ChoiceOption;
use beui_components_unstyled::ChoiceOptionHandle;
use beui_components_unstyled::DragPoint;
use beui_components_unstyled::Scroll;
use beui_components_unstyled::ScrollStyle;
use beui_components_unstyled::back_slide::{BackSlide, SlideCurves};
use beui_components_unstyled::context_menu::ContextMenu;
use beui_components_unstyled::drawn_child;
use beui_components_unstyled::menu::{MenuItem, MenuNodes};
use beui_components_unstyled::menu_popup::MenuStyle;
use beui_components_unstyled::rubber_band::Band;
use beui_components_unstyled::{
    Reorder, ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove, carrying, drag_board,
    use_drop_site,
};
use beui_view::components::overlay::Overlay;
use beui_view::components::root::Root;
use beui_view::drag::Carry;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Image;
use beui_view::types::Placement;
use beui_view::types::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport,
};
use beui_view::types::{Area, Claim, HitRegion, TouchDrags};
use beui_view::types::{CanvasArrangement, Direction};
use beui_view::types::{CursorIcon, Key, KeyPress, Modifiers, PointerPress};
use beui_view::types::{Pos2, Rect, Vec2, pos2, vec2};
use beui_view::window::{
    Window, WindowHandle, opens_windows, set_work_area, use_window, window_origin,
};

pub use bars::DockBarBandHandle;
pub(crate) use bars::{BarCovers, BarFloats};
use bars::{BarHolder, BarPanels, DockBarLifted, DockBarWindows, DockBarred, Lifting};
use compact::DockCompactView;
pub use dialog::{DockDialogHandle, DockDialogSheetHandle};
use dialog::{DialogParts, DockDialogs};
use fullscreen::DockAway;
pub use fullscreen::DockAwayHandle;
pub use hosting::{DockArea, DockHost, DockHostLayout, DockSnap, DockWindowActions, holding_area};
use modifier_drag::{Resized, modifier_resize};
pub use reveal::HiddenBand;
use switch::DockSwitchView;
pub use switch::{DockSwitchHandle, DockSwitchRowHandle};

use beui_dock::{
    DockMenus, DockPaneControl, DockRequest, DockTabControl, DockTabDriver, DockTabMenu,
    dock_menu_items,
};
use beui_dock_state::*;

const EDGE_ZONE: f32 = 0.22;
/// A floating window's resize grips lie wholly outside its rect in a ring this wide, so neither the
/// grips nor the splitters (which take presses only in the gap they draw) reach over content.
pub const HANDLE_REACH: f32 = 8.0;
/// How far each corner's grip runs along both edges.
const GRIP_CORNER: f32 = 24.0;
const SPLIT_STEP: f32 = 0.02;
const SIDEBAR_STEP: f32 = 16.0;
const SIDEBAR_SECONDS: f32 = 0.18;
const FLOAT_INSET: Vec2 = Vec2::new(64.0, 48.0);
const GRAB_OFFSET: Vec2 = Vec2::new(72.0, 14.0);
const OUTER_EDGE: f32 = 18.0;
const WINDOW_KEEP_VISIBLE: f32 = 96.0;

pub struct DockTabHandle {
    pub entry: Entry,
    pub leaf: LeafId,
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
    pub host: Option<TabId>,
    pub test_key: Option<String>,
    pub grouped: bool,
    pub notice: Memo<Option<String>>,
    pub close: Callback<()>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockDragged {
    Entry(Entry),
    Pane(LeafId),
    Bar(TabId),
}

/// What a dock's carry holds: the tab or pane dragged, named with the dock it is from (its
/// `State::id`), so another dock whose tab ids coincide neither takes it nor hides a tab of its
/// own for it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DockCarry {
    dock: u64,
    dragged: DockDragged,
}

/// A splitter carried on the board, named with its dock (`State::id`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SplitterCarry {
    dock: u64,
    split: SplitId,
}

/// What a carried tab or pane is drawn as: floating under the pointer, or, while the dock
/// parts for it (`opening`), the size of the place it would open, naming the tab it would be
/// grouped with if it would (`group_with`).
pub struct DockPreviewHandle {
    pub dragged: DockDragged,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub opening: bool,
    pub group_with: Memo<Option<String>>,
}

pub struct DockChromeHandle {
    pub floating: bool,
    pub barred: bool,
    pub focused: Memo<bool>,
    pub see_through: Memo<bool>,
    pub content: NodeId,
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
    pub close: Callback<()>,
    pub test_prefix: String,
}

pub struct DockStripHandle {
    pub direction: Direction,
    pub tabs: NodeId,
    /// Every tab of the strip as a row that shows it, the shown one chosen: what a face offers
    /// in a menu while the strip is too short for its tabs (row ids `tab.<tab>`, `group.<index>`).
    pub menu: Memo<Vec<MenuNode>>,
    pub test_prefix: String,
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
    pub locked: Memo<bool>,
}

pub struct DockStackHandle {
    pub title: Memo<String>,
    pub notice: Memo<Option<String>>,
    pub icon: Memo<Option<Glyph>>,
    pub backward: Memo<bool>,
    pub menu: Memo<Vec<MenuNode>>,
    pub back: Callback<()>,
    pub open_tabs: Memo<usize>,
    pub switch_label: Memo<String>,
    pub open_switcher: Callback<()>,
}

pub struct DockSwitcherHandle {
    pub open: ReadSignal<bool>,
    pub close: Callback<()>,
    pub tabs: Memo<Vec<TabId>>,
    pub fixed: Memo<Vec<TabId>>,
    pub card: Func<TabId, DockSwitcherCardHandle>,
}

pub struct DockSwitcherCardHandle {
    pub tab: TabId,
    pub test_key: String,
    pub title: Memo<String>,
    pub icon: Memo<Option<Glyph>>,
    pub current: Memo<bool>,
    pub closable: bool,
    pub menu: Memo<Vec<MenuNode>>,
    pub pick: Callback<()>,
    pub close: Callback<()>,
}

type DockNotices = HashMap<TabId, (u64, Memo<Option<String>>)>;

thread_local! {
    static NEXT_NOTICE: Cell<u64> = const { Cell::new(1) };
    static NEXT_DOCK: Cell<u64> = const { Cell::new(1) };
}

#[derive(Clone, Copy)]
struct TabBar {
    strip: NodeRef,
    tabs: NodeRef,
}

#[derive(Clone, Copy)]
struct State {
    state: Binding<DockState>,
    on_close: Callback<TabId>,
    /// This dock among the others in the document, for its carries (`DockCarry`).
    id: u64,
    /// The dock's drop site's `over`, where a carried tab or pane would land, once
    /// `DockTiles` has made the site (`State::previewed`).
    over: Signal<Option<ReadSignal<Option<fluid::DockPlace>>>>,
    /// The place a tab bar parts for a carried tab or pane of this dock, which the rest of
    /// the dock parts for too.
    bar_place: Signal<Option<fluid::DockPlace>>,
    /// A dropped tab or pane gliding from where it was carried into where it landed.
    landing: Signal<Option<fluid::Landing>>,
    /// Where the carried tab or pane was last drawn over the dock, in its window's points.
    ghost_at: StoredCell<Option<(SurfaceId, Rect)>>,
    title: Func<TabId, String>,
    group_title: Func<GroupId, Option<String>>,
    group_host: Func<GroupId, Option<TabId>>,
    icon: Func<TabId, Option<Glyph>>,
    picture: Func<TabId, Option<Image>>,
    closable: Func<TabId, bool>,
    back_closes: Func<TabId, bool>,
    menu: MenuStyle,
    drag_modifier: Memo<Option<Modifiers>>,
    menus: Signal<DockMenus>,
    notices: Signal<DockNotices>,
    offers: Signal<HashMap<TabId, u64>>,
    thickness: f32,
    reach: f32,
    group_inset: f32,
    tab_spacing: f32,
    back_shade: Memo<Color>,
    icon_sidebar_width: f32,
    rect: ReadSignal<Rect>,
    outer: ReadSignal<Rect>,
    dock_body: NodeRef,
    dock_pane: NodeRef,
    opens: ReadSignal<bool>,
    home: Option<WindowHandle>,
    placements: Signal<HashMap<SurfaceId, WindowPlacement>>,
    panes: Stored<RefCell<HashMap<Tree, NodeRef>>>,
    windows: Stored<RefCell<HashMap<SurfaceId, NodeRef>>>,
    /// Each shown dialog's layer, which a carried tab's places count as the dock's own.
    dialog_layers: Stored<RefCell<HashMap<TabId, NodeRef>>>,
    stacked: Stored<RefCell<HashMap<SurfaceId, NodeRef>>>,
    fullscreen_layer: NodeRef,
    fullscreen_window: NodeRef,
    bars: Stored<RefCell<HashMap<LeafId, TabBar>>>,
    /// The panels built now, kept by the dock's `KeepAlive` (`Held::Panel`).
    panels: Signal<HashMap<TabId, NodeId>>,
    /// How many views show each tab's panel now: panes and the fullscreen view, not the back
    /// preview, which only pictures the page a back gesture goes to.
    showing: Stored<RefCell<HashMap<TabId, usize>>>,
    /// The tabs whose own dock joined this one, shown as that dock's tabs.
    hosts: Memo<Vec<TabId>>,
    /// Whether the dock is shown, which its panels read as their `kept_shown()`: what a tab's
    /// content holds outside itself is a dock joined into this one, which a pane showing another
    /// tab, or building its view again, must not make it let go.
    shown: Memo<bool>,
    handles: Signal<HashMap<TabId, (NodeRef, bool)>>,
    owner: Stored<Option<ScopeContext>>,
    tab: RenderFn<DockTabHandle, NodeId>,
    content: RenderFn<TabId, NodeId>,
    empty: RenderFn<LeafId, NodeId>,
    fullscreen: RenderFn<NodeId, NodeId>,
    away: RenderFn<DockAwayHandle, NodeId>,
    chrome: RenderFn<DockChromeHandle, NodeId>,
    bar: RenderFn<DockBarHandle, NodeId>,
    splitter: RenderFn<DockSplitterHandle, NodeId>,
    strip: RenderFn<DockStripHandle, NodeId>,
    grip: RenderFn<DockGripHandle, NodeId>,
    preview: RenderFn<DockPreviewHandle, NodeId>,
    stack: Option<RenderFn<DockStackHandle, NodeId>>,
    switcher: Option<RenderFn<DockSwitcherHandle, NodeId>>,
    switch: Option<RenderFn<DockSwitchHandle, NodeId>>,
    mode: Memo<DockMode>,
    dialogs: Memo<Vec<(TabId, TabId)>>,
    dialog: Func<TabId, Option<DialogParts>>,
    dialog_chrome: RenderFn<DockDialogHandle, NodeId>,
    dialog_sheet: Option<RenderFn<DockDialogSheetHandle, NodeId>>,
    dialog_beak: f32,
    curves: DockCurves,
    dialog_radius: f32,
    dialog_scrim: Memo<Color>,
    window_border: f32,
    dialog_frames: Stored<RefCell<HashMap<TabId, NodeRef>>>,
    dialog_bodies: Stored<RefCell<HashMap<TabId, NodeRef>>>,
    dialog_focus: Signal<Option<TabId>>,
    attention: Signal<(u64, Option<TabId>)>,
    given: Signal<(u64, Option<TabId>)>,
    forward: Signal<(u64, Option<TabId>)>,
    remembered: Stored<RefCell<HashMap<TabId, NodeId>>>,
    content_keyboard: StoredCell<Option<TabId>>,
    /// A tab moved while the focus was on it in its bar (`State::focus_on_tab`), by the keyboard
    /// or a pointer whose press focused it, and active in its pane after the move: that pane
    /// gives its panel the keyboard as it shows it, since the tab's row is built again
    /// elsewhere. Set by the move's edit and cleared by the next (`State::edit`).
    keyboard_after_move: StoredCell<Option<TabId>>,
    /// The floating windows the focus is in.
    focused_windows: Signal<Vec<SurfaceId>>,
    keyboard: ReadSignal<bool>,
    bar_panels: Stored<BarPanels>,
    bodies: Stored<RefCell<HashMap<LeafId, (NodeRef, NodeRef)>>>,
    bar_windows: Stored<RefCell<HashMap<String, WindowHandle>>>,
    bar_placements: Signal<HashMap<String, WindowPlacement>>,
    bar_key: Func<TabId, Option<String>>,
    tab_key: Func<TabId, Option<String>>,
    /// A bar the dock carries for a press it relays (`PointerId::Relayed`: an app's grip, or a
    /// grip in the window the bar was popped into), and where the bar was laid out at the lift.
    relayed: Signal<Option<(TabId, Rect)>>,
    bar_margin: f32,
    covers: Signal<BarCovers>,
    floats: Signal<BarFloats>,
    covered: Signal<bool>,
    reveal: reveal::BandReveal,
    bars_away: Memo<bool>,
    bar_band: RenderFn<DockBarBandHandle, NodeId>,
    lifting: Lifting,
    dragged_edits: Stored<RefCell<Vec<Box<dyn FnOnce(&mut DockState)>>>>,
}

impl DockTabDriver for State {
    fn handle(&self, tab: TabId) -> Option<NodeId> {
        if !self.panels.is_alive() {
            return None;
        }
        let mut tab = tab;
        let mut seen = Vec::new();
        loop {
            let held = self
                .handles
                .with(|handles| handles.get(&tab).and_then(|(node, _)| node.try_get()));
            if let Some(node) = held {
                return Some(node);
            }
            seen.push(tab);
            tab = self
                .dialog_parent(tab)
                .filter(|parent| !seen.contains(parent))?;
        }
    }

    fn fullscreen(&self, tab: TabId) -> bool {
        self.state
            .with(|state| state.fullscreen().is_some_and(|shown| shown.tab == tab))
    }

    fn enter_fullscreen(&self, tab: TabId, area: Option<Rect>) {
        if self.panels.is_alive() {
            self.edit(|state| state.set_fullscreen(Some(DockFullscreen { tab, area })));
        }
    }

    fn leave_fullscreen(&self, tab: TabId) {
        if self.panels.is_alive() {
            State::leave_fullscreen(self, tab);
        }
    }

    fn offer_fullscreen(&self, tab: TabId) {
        if !self.panels.is_alive() || untrack(|| self.is_dialog(tab)) {
            return;
        }
        let key = NEXT_NOTICE.with(|next| next.replace(next.get() + 1));
        let offers = self.offers;
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

    fn focused(&self, tab: TabId) -> bool {
        let focused_tab = || self.state.with(|state| state.focused_tab() == Some(tab));
        if !self.panels.is_alive() {
            return focused_tab();
        }
        match self.dialog_focus.get() {
            Some(dialog) => dialog == tab,
            None => focused_tab(),
        }
    }

    fn attention(&self, tab: TabId) -> Option<u64> {
        if !self.panels.is_alive() {
            return None;
        }
        self.attention
            .with(|(count, target)| (*target == Some(tab)).then_some(*count))
    }

    fn show(&self, tab: TabId) {
        if !self.panels.is_alive() {
            return;
        }
        match self.is_dialog(tab) {
            true => self.focus_dialog(tab),
            false => {
                State::show(self, tab);
                self.release_dialog_focus();
            }
        }
    }

    fn content_focused(&self, tab: TabId) {
        if !self.panels.is_alive() {
            return;
        }
        // A view of a tab's content that takes the keyboard while no view shows its built panel
        // (an app putting it back on a popup's trigger as a press on another tab closes the
        // popup) does not show the tab again; a tab whose own dock joined shows as its tabs.
        let built = self
            .panels
            .with_untracked(|panels| panels.contains_key(&tab));
        let showing = self
            .showing
            .value()
            .borrow()
            .get(&tab)
            .copied()
            .unwrap_or(0);
        if built && showing == 0 && !self.hosts.with_untracked(|hosts| hosts.contains(&tab)) {
            return;
        }
        self.content_keyboard.set(Some(tab));
        DockTabDriver::show(self, tab);
    }

    fn show_notice(&self, tab: TabId, notice: Memo<Option<String>>) {
        if !self.panels.is_alive() {
            return;
        }
        let key = NEXT_NOTICE.with(|next| next.replace(next.get() + 1));
        let notices = self.notices;
        notices.update(|notices| {
            notices.insert(tab, (key, notice));
        });
        on_cleanup(move || {
            notices.update(|notices| {
                if notices.get(&tab).is_some_and(|(held, _)| *held == key) {
                    notices.remove(&tab);
                }
            });
        });
    }

    fn window_size(&self, tab: TabId) -> Option<Vec2> {
        if !self.panels.is_alive() {
            return None;
        }
        self.state.with_untracked(|state| {
            let surface = state.find(tab)?.surface;
            (state.surface_tabs(surface) == [tab]).then_some(())?;
            state.window_rect(surface).map(|rect| rect.size())
        })
    }

    fn resize_window(&self, tab: TabId, size: Vec2) {
        if !self.panels.is_alive() {
            return;
        }
        self.edit(|state| {
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

    fn brought_forward(&self, tab: TabId) -> Option<u64> {
        if !self.panels.is_alive() {
            return None;
        }
        self.forward
            .with(|(count, target)| (*target == Some(tab)).then_some(*count))
    }

    fn keyboard(&self, tab: TabId) -> Option<u64> {
        if !self.panels.is_alive() {
            return None;
        }
        self.given
            .with(|(count, target)| (*target == Some(tab)).then_some(*count))
    }
}

impl State {
    fn tab_control(&self, tab: TabId) -> DockTabControl {
        DockTabControl::local(tab, Rc::new(*self))
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        // A move's hand-over of the keyboard lasts only for the edit that made it.
        self.keyboard_after_move.set(None);
        let current = self.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next == current {
            return;
        }
        self.state.set(next);
    }

    fn drag_edit(&self, change: impl FnOnce(&mut DockState) + 'static) {
        let queued = self.dragged_edits.value();
        let mut queued = queued.borrow_mut();
        queued.push(Box::new(change));
        if queued.len() > 1 {
            return;
        }
        let dock = *self;
        after_input(move || {
            let Some(changes) = dock
                .dragged_edits
                .try_with(|edits| std::mem::take(&mut *edits.borrow_mut()))
            else {
                return;
            };
            dock.edit(|state| {
                for change in changes {
                    change(state);
                }
            });
        });
    }

    fn test_key(&self, tab: TabId) -> String {
        untrack(|| self.tab_key.call(tab)).unwrap_or_else(|| tab.value().to_string())
    }

    /// A tab's title; an untitled one (an editor's stage) is named after the tab it stands in for.
    fn title(&self, tab: TabId) -> String {
        let title = self.title.call(tab);
        match title.is_empty() {
            true => match self.standing(tab) {
                standing if standing != tab => self.title.call(standing),
                _ => title,
            },
            false => title,
        }
    }

    pub(crate) fn notice(&self, tab: TabId) -> Option<String> {
        self.notices
            .with(|notices| notices.get(&tab).map(|(_, notice)| *notice))
            .filter(|notice| notice.is_alive())
            .and_then(|notice| notice.get())
    }

    /// Whether a bar offers to close its pane or window: when every tab it shows is closable.
    fn all_closable(&self, leaves: impl Fn(&DockState) -> Vec<LeafId> + 'static) -> Memo<bool> {
        let dock = *self;
        create_memo(move || {
            let entries = dock.state.with(|state| {
                leaves(state)
                    .into_iter()
                    .flat_map(|leaf| state.entries(leaf))
                    .collect::<Vec<_>>()
            });
            !entries.is_empty() && entries.iter().all(|entry| dock.entry_closable(*entry))
        })
    }

    fn icon(&self, tab: TabId) -> Option<Glyph> {
        self.icon.call(tab)
    }

    /// The tab `tab` stands in for (`DockState::standing_in`): a joined editor's stage names, counts
    /// and closes as its document's tab.
    fn standing(&self, tab: TabId) -> TabId {
        self.state.with(|state| state.standing_in(tab))
    }

    fn standing_closable(&self, tab: TabId) -> bool {
        self.closable.call(self.standing(tab))
    }

    fn standing_tab(&self, entry: Entry) -> Option<TabId> {
        match entry {
            Entry::Tab(tab) => Some(tab),
            Entry::Group(group) => self.state.with(|state| state.stands_for(group)),
        }
    }

    fn entry_picture(&self, entry: Entry) -> Option<Image> {
        self.standing_tab(entry)
            .and_then(|tab| self.picture.call(tab))
    }

    fn entry_icon(&self, entry: Entry) -> Option<Glyph> {
        self.standing_tab(entry).and_then(|tab| self.icon(tab))
    }

    fn dragged_icon(&self, dragged: DockDragged) -> Option<Glyph> {
        match dragged {
            DockDragged::Entry(entry) => self.entry_icon(entry),
            DockDragged::Bar(_) => None,
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
            let entry = leaf.and_then(|leaf| dock.state.with(|state| state.active_entry(leaf)));
            let inner = leaf.and_then(|leaf| dock.state.with(|state| state.menu_tab(leaf)));
            let host = entry.and_then(|entry| dock.standing_tab(entry));
            let mut items = dock_menu_items(dock.menus.read_only(), inner);
            if host != inner {
                // The group's host and the tab inside it may offer the same entries (a
                // workspace adds its block's References and Access to both): each shows once.
                let listed: be_lib_stable_hash::HashSet<String> = items
                    .iter()
                    .filter_map(|node| node.id().map(str::to_owned))
                    .collect();
                let held: Vec<MenuNode> = dock_menu_items(dock.menus.read_only(), host)
                    .into_iter()
                    .filter(|node| node.id().is_none_or(|id| !listed.contains(id)))
                    .collect();
                items.push(MenuNode::Separator);
                items.extend(held);
            }
            tidy_menu(items)
        })
    }

    /// Counts a view showing `tab`'s panel for as long as the scope it is called in lasts.
    fn count_showing(&self, tab: TabId) {
        *self.showing.value().borrow_mut().entry(tab).or_default() += 1;
        let showing = self.showing;
        on_cleanup(move || {
            if !showing.is_alive() {
                return;
            }
            let held = showing.value();
            let mut showing = held.borrow_mut();
            if let Some(count) = showing.get_mut(&tab) {
                *count -= 1;
                if *count == 0 {
                    showing.remove(&tab);
                }
            }
        });
    }

    /// Builds `tab`'s panel, in the scope of the `Kept` that keeps it (`DockPanel`).
    fn build_panel(&self, tab: TabId) -> NodeId {
        let control = self.tab_control(tab);
        provide_context(DockTabMenu {
            tab,
            menu: self.menus,
        });
        provide_context(control);
        provide_kept_shown(self.shown);
        beui_components_unstyled::provide_frame_anchor(create_memo(move || control.anchor()));
        let panel = self.content.call(tab);
        untrack(|| {
            self.panels.update(|panels| {
                panels.insert(tab, panel);
            })
        });
        let (panels, remembered) = (self.panels, self.remembered);
        on_cleanup(move || {
            if panels.is_alive() {
                panels.update(|panels| {
                    panels.remove(&tab);
                });
            }
            if remembered.is_alive() {
                remembered.value().borrow_mut().remove(&tab);
            }
        });
        panel
    }

    fn open_scope(&self) -> Scope {
        self.owner
            .with(|owner| owner.as_ref().and_then(ScopeContext::child))
            .unwrap_or_else(Scope::detached)
    }

    fn pane_rect(&self, tree: Tree) -> Option<Rect> {
        let node = self.panes.value().borrow().get(&tree)?.try_get()?;
        Some(measure(node).get_untracked()?.rect)
    }

    fn raise_window(&self, window: NodeRef) {
        self.raise_window_of(window, None);
    }

    fn raise_window_of(&self, window: NodeRef, surface: Option<SurfaceId>) {
        let Some(window) = window.try_get() else {
            return;
        };
        let holds = surface.is_some_and(|surface| {
            self.state.with_untracked(|state| {
                state
                    .fullscreen()
                    .is_some_and(|shown| state.surface_tabs(surface).contains(&shown.tab))
            })
        });
        let in_front = holds || self.fullscreen_in_front();
        window.raise_overlay();
        for fullscreen in [self.fullscreen_layer, self.fullscreen_window] {
            if let Some(fullscreen) = fullscreen.try_get().filter(|_| in_front) {
                fullscreen.raise_overlay();
            }
        }
    }

    fn restack(&self, from: SurfaceId) {
        let order = self.state.with_untracked(DockState::windows);
        let Some(at) = order.iter().position(|surface| *surface == from) else {
            return;
        };
        let raised: Vec<(SurfaceId, NodeRef)> = self
            .stacked
            .value()
            .borrow()
            .iter()
            .map(|(surface, overlay)| (*surface, *overlay))
            .collect();
        for surface in &order[at..] {
            if let Some((_, overlay)) = raised.iter().find(|(held, _)| held == surface) {
                self.raise_window_of(*overlay, Some(*surface));
            }
        }
    }

    fn surface_rect(&self, surface: SurfaceId) -> Option<Rect> {
        let window = self
            .windows
            .value()
            .borrow()
            .get(&surface)
            .and_then(NodeRef::try_get);
        match window {
            Some(window) => Some(measure(window).get_untracked()?.rect),
            None => self.pane_rect(Tree::Surface(surface)),
        }
    }

    fn window_bar(&self, surface: SurfaceId) -> Option<HitRegion> {
        let pane = self
            .panes
            .value()
            .borrow()
            .get(&Tree::Surface(surface))?
            .try_get()?;
        let window = self
            .windows
            .value()
            .borrow()
            .get(&surface)
            .and_then(NodeRef::try_get)
            .unwrap_or(pane);
        let mut bar = HitRegion::rect(tracked_rect(window)?).without(tracked_rect(pane)?);
        for leaf in self.state.with(|state| state.leaves(surface)) {
            let tabs = self
                .bars
                .value()
                .borrow()
                .get(&leaf)
                .and_then(|bar| bar.tabs.try_get());
            for tab in tabs.map(|tabs| tabs.children()).unwrap_or_default() {
                if let Some(rect) = tracked_rect(drawn_child(tab).unwrap_or(tab)) {
                    bar = bar.without(rect);
                }
            }
        }
        Some(bar)
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
        // Where each tab is drawn: one still gliding into place is drawn off its laid-out
        // rect, which then shows nothing.
        node.children()
            .into_iter()
            .filter_map(|tab| {
                Some(
                    measure(drawn_child(tab).unwrap_or(tab))
                        .get_untracked()?
                        .rect,
                )
            })
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
        Some(measure(node).get_untracked()?.rect)
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

    fn splitter_test_id(&self, split: SplitId) -> String {
        match self
            .state
            .with_untracked(|state| state.split_key(split).map(str::to_owned))
        {
            Some(key) => format!("dock.splitter.{key}"),
            None => "dock.splitter".to_owned(),
        }
    }

    fn toggle_icons_only(&self, leaf: LeafId) {
        self.edit(|state| {
            let icons_only = state.is_icons_only(leaf);
            state.set_icons_only(leaf, !icons_only);
        });
    }

    fn toggle_lock(&self, leaf: LeafId) {
        self.edit(|state| {
            if let Some(place) = state.lock_of(leaf) {
                let locked = state.is_locked(&place);
                state.set_locked(place, !locked);
            }
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
        let ease = self.curves.ease;
        let animation = create_animation(
            move |frame| {
                let progress = tween(frame.elapsed.as_secs_f32(), SIDEBAR_SECONDS, ease);
                let start = from.get();
                shown.set(start + (target.get_untracked() - start) * progress);
                progress < 1.0
            },
            move || shown.set(target.get_untracked()),
        );
        create_effect_on(
            move || (target.get(), icons_only.get()),
            move |&(width, icons_only), previous| {
                if previous.is_some_and(|&(_, was)| was != icons_only) {
                    from.set(shown.get_untracked());
                    animation.stop();
                    animation.start();
                } else if !animation.running() {
                    shown.set(width);
                }
            },
        );
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
        visit.opened && self.closable.call(visit.to) && self.back_closes.call(visit.to)
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

    /// Whether `close_entry` would close anything: the host tab of a pinned group, else every
    /// tab of the entry.
    fn entry_closable(&self, entry: Entry) -> bool {
        if let Entry::Group(group) = entry
            && self.state.with(|state| state.is_pinned(group))
        {
            return untrack(|| self.group_host.call(group))
                .is_some_and(|host| self.closable.call(host));
        }
        let tabs = self.state.with(|state| state.entry_tabs(entry));
        !tabs.is_empty() && tabs.iter().all(|tab| self.closable.call(*tab))
    }

    fn close_entry(&self, entry: Entry) {
        if let Entry::Group(group) = entry
            && self.state.with_untracked(|state| state.is_pinned(group))
        {
            if let Some(host) = untrack(|| self.group_host.call(group)) {
                self.close_tab(host);
            }
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
            .and_then(|window| window.real().get_untracked())
            .unwrap_or(WindowId::MAIN)
    }

    fn watch_home(&self) -> Memo<WindowId> {
        let home = self.home;
        create_memo(move || {
            home.and_then(|window| window.real().get())
                .unwrap_or(WindowId::MAIN)
        })
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
            DockDragged::Bar(_) => {}
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
        let placed = self
            .state
            .with(|state| state.window_placed(surface).map(str::to_owned));
        if let Some(placed) = placed {
            return WindowPlacement::Host(HostPlacement(placed));
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
            WindowReport::Placed { placement, .. } => {
                self.placements.update(|placements| {
                    placements.remove(&surface);
                });
                self.edit(|state| state.set_window_placed(surface, placement.0));
            }
            WindowReport::Focused { focused: true, .. } => self.window_focused(surface),
            WindowReport::Rastered { .. }
            | WindowReport::Moved { .. }
            | WindowReport::Focused { .. } => {}
            WindowReport::Closed(_) => {
                for tab in self.state.get_untracked().surface_tabs(surface) {
                    self.close_tab(tab);
                }
            }
        }
    }

    /// The host focused this window with nothing in it to give the focus back to: focus its pane and,
    /// unless the keyboard is already inside, hand it to the shown tab. So a tab floated into a window the
    /// host focuses on opening takes the keyboard with it, while the main window keeps its own focus for
    /// when it is focused again.
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
        rect.kept_in(rect.size(), bounds)
    }

    fn cycle(&self, backwards: bool) {
        let state = self.state.get_untracked();
        let Some(leaf) = state.focused_leaf().map(|leaf| state.standing_leaf(leaf)) else {
            return;
        };
        let count = state.entries(leaf).len();
        if count < 2 {
            return;
        }
        let index = state.active_index(leaf);
        let next = match backwards {
            true => (index + count - 1) % count,
            false => (index + 1) % count,
        };
        // An entry that is a joined editor's group shows a body built anew, which hands on no
        // keyboard from the one it replaces as a pane switching tabs does (`DockShownBody`): the
        // keyboard moves here, from the entry that held it to the one shown.
        let leaving = state.active_entry(leaf);
        let groups = [leaving, state.entries(leaf).get(next).copied()]
            .into_iter()
            .flatten()
            .any(|entry| entry.group().is_some());
        let held = groups && leaving.is_some_and(|entry| self.entry_holds_keyboard(&state, entry));
        self.edit(|state| state.set_active_index(leaf, next));
        let taker = self.state.with_untracked(|state| shown_tab(state, leaf));
        if held && let Some(tab) = taker {
            self.give_keyboard(tab);
        }
    }

    /// The focused pane and the entry it shows: for a joined editor's stage, the pane its group
    /// stands in and the group (`DockState::standing_leaf`), so Close tab and Split act on the
    /// document, as Next tab walks the documents.
    fn shown_entry(&self) -> Option<(LeafId, Entry)> {
        self.state.with(|state| {
            let leaf = state.standing_leaf(state.focused_leaf()?);
            Some((leaf, state.active_entry(leaf)?))
        })
    }

    fn split_shown(&self, side: Side) {
        if let Some((leaf, entry)) = untrack(|| self.shown_entry()) {
            self.edit(|state| state.drop_entry(entry, DockDrop::Split { leaf, side }));
        }
    }

    fn dragged_title(&self, dragged: DockDragged) -> String {
        match dragged {
            DockDragged::Entry(entry) => self.entry_title(entry),
            DockDragged::Bar(_) => String::new(),
            DockDragged::Pane(leaf) => {
                let tabs = self.state.with(|state| {
                    state
                        .entries(leaf)
                        .into_iter()
                        .flat_map(|entry| state.standing_tabs(entry))
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

    fn bar_admits(&self, leaf: LeafId, dragged: DockDragged) -> bool {
        self.state.with_untracked(|state| {
            let target = DockDrop::Tab { leaf, index: 0 };
            match dragged {
                DockDragged::Entry(entry) => {
                    !state.contains_leaf(entry, leaf) && state.admits(entry, target)
                }
                DockDragged::Pane(pane) => {
                    pane != leaf
                        && !state
                            .entries(pane)
                            .into_iter()
                            .any(|entry| state.contains_leaf(entry, leaf))
                        && state.admits_leaf(pane, target)
                }
                DockDragged::Bar(_) => false,
            }
        })
    }

    /// The tab or pane this dock carries, if the board carries one of it (a tool bar
    /// carried at once is `carried_bar`'s).
    fn carried(&self) -> Option<DockDragged> {
        carrying::<DockCarry>()
            .into_iter()
            .filter(|carry| carry.dock == self.id)
            .map(|carry| carry.dragged)
            .find(|dragged| !matches!(dragged, DockDragged::Bar(_)))
    }

    /// Whether the board carries `dragged` of this dock; tracked.
    fn carries(&self, dragged: DockDragged) -> bool {
        drag_board().carries(&self.carry(dragged))
    }

    fn carry(&self, dragged: DockDragged) -> DockCarry {
        DockCarry {
            dock: self.id,
            dragged,
        }
    }

    fn move_into_bar(&self, leaf: LeafId, moved: ReorderMove<Entry, DockCarry>) {
        // A tab moved within its own bar keeps its row, and the focus on it.
        let moving = match moved.payload.dragged {
            DockDragged::Entry(entry)
                if self
                    .state
                    .with_untracked(|state| state.locate(entry).map(|(at, _)| at))
                    != Some(leaf) =>
            {
                self.focus_moving_with(entry)
            }
            _ => None,
        };
        self.edit(|state| {
            let entries = state.entries(leaf);
            match moved.payload.dragged {
                DockDragged::Entry(entry) => {
                    let index = bar_index(&entries, entry, moved.after).unwrap_or(moved.index);
                    state.drop_entry(entry, DockDrop::Tab { leaf, index });
                }
                DockDragged::Pane(pane) => {
                    let index = match moved.after {
                        None => 0,
                        Some(after) => entries
                            .iter()
                            .position(|entry| *entry == after)
                            .map_or(moved.index, |at| at + 1),
                    };
                    state.drop_leaf(pane, DockDrop::Tab { leaf, index });
                }
                DockDragged::Bar(_) => {}
            }
            self.keyboard_follows_move(state, moving);
        });
    }

    fn splitter_of(&self, tree: Tree, split: SplitId) -> Option<DockSplitter> {
        let area = self.pane_rect(tree)?;
        let state = self.state.get_untracked();
        layout_tree(&state, tree, area, self.thickness).splitter(split)
    }
}

fn sidebar_length(vertical: bool, width: Memo<f32>) -> Prop<Option<f32>> {
    match vertical {
        true => width.into_prop().map(Some),
        false => Prop::Static(None),
    }
}

fn middle_target(state: &DockState, leaf: LeafId) -> DockDrop {
    match state.active_entry(leaf) {
        Some(_) => DockDrop::Group {
            leaf,
            index: state.active_index(leaf),
        },
        None => DockDrop::Tab { leaf, index: 0 },
    }
}

fn bar_index(entries: &[Entry], entry: Entry, after: Option<Entry>) -> Option<usize> {
    let from = entries.iter().position(|shown| *shown == entry);
    if after == Some(entry) {
        return from;
    }
    let at = match after {
        None => 0,
        Some(after) => {
            entries
                .iter()
                .filter(|shown| **shown != entry)
                .position(|shown| *shown == after)?
                + 1
        }
    };
    Some(match from {
        Some(from) if at > from => at + 1,
        _ => at,
    })
}

fn group_title(titles: Vec<String>) -> String {
    match titles.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first}, {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} +{}", rest.len()),
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

    fn rect(self, upright: bool, size: Vec2, border: f32) -> Rect {
        let (reach, corner) = (HANDLE_REACH + border, GRIP_CORNER);
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
    pub(crate) group_host: Func<GroupId, Option<TabId>>,
    pub(crate) hosts: Memo<Vec<TabId>>,
    pub(crate) icon: Func<TabId, Option<Glyph>>,
    pub(crate) picture: Func<TabId, Option<Image>>,
    pub(crate) closable: Func<TabId, bool>,
    pub(crate) back_closes: Func<TabId, bool>,
    pub(crate) menu: MenuStyle,
    pub(crate) stack_below: f32,
    pub(crate) short_below: f32,
    pub(crate) short_narrower_than: f32,
    pub(crate) mode: Signal<DockMode>,
    pub(crate) drag_modifier: Prop<Option<Modifiers>>,
    pub(crate) tab_keys: Prop<DockTabKeys>,
    pub(crate) splitter_thickness: f32,
    pub(crate) splitter_reach: f32,
    pub(crate) group_inset: f32,
    pub(crate) tab_spacing: f32,
    pub(crate) back_shade: Memo<Color>,
    pub(crate) icon_sidebar_width: f32,
    pub(crate) inset: Prop<f32>,
    pub(crate) tab: RenderFn<DockTabHandle, NodeId>,
    pub(crate) content: RenderFn<TabId, NodeId>,
    pub(crate) empty: RenderFn<LeafId, NodeId>,
    pub(crate) fullscreen: Option<RenderFn<NodeId, NodeId>>,
    pub(crate) away: Option<RenderFn<DockAwayHandle, NodeId>>,
    pub(crate) chrome: Option<RenderFn<DockChromeHandle, NodeId>>,
    pub(crate) bar: Option<RenderFn<DockBarHandle, NodeId>>,
    pub(crate) splitter: Option<RenderFn<DockSplitterHandle, NodeId>>,
    pub(crate) strip: Option<RenderFn<DockStripHandle, NodeId>>,
    pub(crate) grip: Option<RenderFn<DockGripHandle, NodeId>>,
    pub(crate) preview: Option<RenderFn<DockPreviewHandle, NodeId>>,
    pub(crate) stack: Option<RenderFn<DockStackHandle, NodeId>>,
    pub(crate) switcher: Option<RenderFn<DockSwitcherHandle, NodeId>>,
    pub(crate) switch: Option<RenderFn<DockSwitchHandle, NodeId>>,
    pub(crate) dialogs: Memo<Vec<(TabId, TabId)>>,
    pub(crate) dialog: Func<TabId, Option<DialogParts>>,
    pub(crate) dialog_chrome: Option<RenderFn<DockDialogHandle, NodeId>>,
    pub(crate) dialog_sheet: Option<RenderFn<DockDialogSheetHandle, NodeId>>,
    pub(crate) dialog_beak: f32,
    pub(crate) curves: DockCurves,
    pub(crate) dialog_radius: f32,
    pub(crate) dialog_scrim: Memo<Color>,
    pub(crate) window_border: f32,
    pub(crate) bar_margin: f32,
    pub(crate) covers: Signal<BarCovers>,
    pub(crate) floats: Signal<BarFloats>,
    pub(crate) covered: Signal<bool>,
    pub(crate) reveal: reveal::BandReveal,
    pub(crate) area: Signal<Rect>,
    pub(crate) bar_band: Option<RenderFn<DockBarBandHandle, NodeId>>,
    pub(crate) bars_away: Memo<bool>,
    pub(crate) bar_key: Func<TabId, Option<String>>,
    pub(crate) tab_key: Func<TabId, Option<String>>,
}

#[component]
pub(crate) fn Dock(config: DockConfig) -> NodeId {
    let DockConfig {
        api,
        state,
        on_close,
        title,
        group_title,
        group_host,
        hosts,
        icon,
        picture,
        closable,
        back_closes,
        menu,
        stack_below,
        short_below,
        short_narrower_than,
        mode: published,
        drag_modifier,
        tab_keys,
        splitter_thickness,
        splitter_reach,
        group_inset,
        tab_spacing,
        back_shade,
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
        strip,
        grip,
        preview,
        stack,
        switcher,
        switch,
        dialogs,
        dialog,
        dialog_chrome,
        dialog_sheet,
        dialog_beak,
        curves,
        dialog_radius,
        dialog_scrim,
        window_border,
        bar_margin,
        bar_band,
        covers,
        floats,
        covered,
        reveal,
        area: dock_rect_held,
        bars_away,
        bar_key,
        tab_key,
    } = config;
    let dock_rect = dock_rect_held.read_only();
    let outer = create_signal(Rect::ZERO);
    let mode = create_memo(move || {
        let size = outer.get().size();
        let short = size.y > 0.0 && size.y < short_below && size.x < short_narrower_than;
        let compact = size.x > 0.0 && (size.x < stack_below || short);
        if !compact {
            return DockMode::Tiled;
        }
        let staged = state.with(|state| {
            state.is_one_bare_pane() || state.stage_of(Tree::Surface(state.main())).is_some()
        });
        match staged {
            true => DockMode::Compact,
            false => DockMode::Stacked,
        }
    });
    let stacks = create_memo(move || mode.get() == DockMode::Stacked);
    create_effect(move || published.set(mode.get()));
    let dialog_focus = create_signal(None);
    let attention = create_signal((0, None));
    let given = create_signal((0, None));
    let forward = create_signal((0, None));
    let menus = create_signal(DockMenus::default());
    let notices = create_signal(DockNotices::default());
    let offers = create_signal(HashMap::default());
    let id = NEXT_DOCK.with(|next| next.replace(next.get() + 1));
    let carried_bar = create_memo(move || {
        carrying::<DockCarry>()
            .into_iter()
            .filter(|carry| carry.dock == id)
            .find_map(|carry| match carry.dragged {
                DockDragged::Bar(bar) => Some(bar),
                _ => None,
            })
    });
    let dock = State {
        state,
        windows: store(RefCell::default()),
        dialog_layers: store(RefCell::default()),
        stacked: store(RefCell::default()),
        fullscreen_layer: NodeRef::new(),
        fullscreen_window: NodeRef::new(),
        panels: create_signal(HashMap::default()),
        showing: store(RefCell::default()),
        hosts,
        shown: kept_shown().unwrap_or_else(|| create_memo(|| true)),
        handles: create_signal(HashMap::default()),
        owner: store(owner_scope()),
        on_close,
        id,
        over: create_signal(None),
        bar_place: create_signal(None),
        landing: create_signal(None),
        ghost_at: StoredCell::new(None),
        title,
        group_title,
        group_host,
        icon,
        picture,
        closable,
        back_closes,
        menu,
        drag_modifier: create_memo(move || drag_modifier.get()),
        menus,
        notices,
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
        dialog_beak,
        curves,
        dialog_radius,
        dialog_scrim,
        window_border,
        dialog_frames: store(RefCell::default()),
        dialog_bodies: store(RefCell::default()),
        dialog_focus,
        attention,
        given,
        forward,
        remembered: store(RefCell::default()),
        content_keyboard: StoredCell::new(None),
        keyboard_after_move: StoredCell::new(None),
        focused_windows: create_signal(Vec::new()),
        keyboard: use_document_keyboard(),
        bar_panels: store(RefCell::default()),
        bodies: store(RefCell::default()),
        bar_windows: store(RefCell::default()),
        bar_placements: create_signal(HashMap::default()),
        bar_key,
        tab_key,
        relayed: create_signal(None),
        bar_margin,
        covers,
        floats,
        covered,
        reveal,
        bars_away,
        bar_band: bar_band
            .unwrap_or_else(|| RenderFn::new(|handle: DockBarBandHandle| handle.body)),
        lifting: Lifting::new(state, carried_bar),
        dragged_edits: store(RefCell::default()),
        thickness: splitter_thickness,
        reach: splitter_reach,
        group_inset,
        tab_spacing,
        back_shade,
        icon_sidebar_width,
        rect: dock_rect,
        outer: outer.read_only(),
        dock_body: NodeRef::new(),
        dock_pane: NodeRef::new(),
        opens: opens_windows(),
        home: use_window(),
        placements: create_signal(HashMap::default()),
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
        strip: strip.unwrap_or_else(|| {
            RenderFn::new(move |handle: DockStripHandle| {
                let DockStripHandle {
                    direction, tabs, ..
                } = handle;
                view! {
                    <Scroll direction style={ScrollStyle::new(curves.rubber)}>{tabs}</Scroll>
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
        preview: preview.unwrap_or_else(|| {
            RenderFn::new(|_| {
                view! {
                    <Frame />
                }
            })
        }),
    };
    dock.bring_forward_when_shown();
    dock.forget_content_keyboard_when_taken();
    api.fill(DockApi(dock));
    let area = NodeRef::new();
    tab_actions(dock, tab_keys, area);
    let held = create_memo(move || {
        let mut tabs = state.with(DockState::all_tabs);
        tabs.extend(hosts.get());
        let mut held: Vec<Held> = tabs.into_iter().map(Held::Panel).collect();
        held.extend(state.with(DockState::bars).into_iter().map(Held::Bar));
        held
    });
    // A tab whose dock joined this one shows as that dock's tabs, so its panel keeps running (read
    // from the state too, which drops the tab from the layout in the same change that joins its
    // group, before `hosts` may hear of it); so does every bar's, whose content may answer keys
    // while its band is hidden (Super bringing out an auto-hidden apps bar).
    let awake = create_memo(move || {
        let mut awake: Vec<Held> = hosts.get().into_iter().map(Held::Panel).collect();
        state.with(|state| {
            awake.extend(
                state
                    .joined_groups()
                    .into_iter()
                    .map(|(_, host)| Held::Panel(host)),
            );
        });
        awake.extend(state.with(DockState::bars).into_iter().map(Held::Bar));
        awake
    });
    let built = view! {
        <KeepAlive keep={held} awake={awake}>
            <List @node_ref=&area spacing=0.0>
                <ListItem grow=1.0>
                    <DockBarred dock leaf={None} holder={BarHolder::Dock}>
                        <Frame padding=inset>
                            <List spacing=0.0>
                                <Show
                                    value={stacks}
                                    fallback={move || view! {
                                        <ListItem grow=1.0>
                                            <DockTiles dock />
                                        </ListItem>
                                    }}
                                >
                                    <ListItem grow=1.0>
                                        <DockStack dock />
                                    </ListItem>
                                </Show>
                            </List>
                        </Frame>
                    </DockBarred>
                </ListItem>
                <DockFullscreenView dock={dock} />
                <DockDialogs dock={dock} />
                <DockSwitchView dock={dock} />
            </List>
        </KeepAlive>
    };
    let outer_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = outer_measured.get() {
            outer.set(measured.rect);
        }
    });
    if let Some(body) = dock.dock_body.try_get() {
        let body_measured = measure(body);
        create_effect(move || {
            if let Some(measured) = body_measured.get() {
                dock_rect_held.set(measured.rect);
            }
        });
    }
    share_work_area(dock);
    built
}

fn share_work_area(dock: State) {
    let home = dock.watch_home();
    let opens = dock.opens;
    let area = dock.rect;
    let shared = Rc::new(Cell::new(None::<WindowId>));
    let held = Rc::clone(&shared);
    create_effect(move || {
        let window = home.get();
        let area = area.get();
        let shown = opens.get() && area.is_positive();
        if let Some(was) = held.replace(shown.then_some(window))
            && was != window
        {
            set_work_area(was, None);
        }
        set_work_area(window, shown.then_some(area));
    });
    on_cleanup(move || {
        if let Some(window) = shared.take() {
            set_work_area(window, None);
        }
    });
}

#[component]
fn DockFullscreenView(dock: State) -> NodeId {
    let state = dock.state;
    let fullscreen = create_memo(move || state.with(DockState::fullscreen));
    let panel = create_memo(move || fullscreen.get().map(|shown| shown.tab));
    let open = create_memo(move || fullscreen.with(Option::is_some));
    let area = create_memo(move || fullscreen.get().and_then(|shown| shown.area));
    let placement = create_memo(move || match area.get() {
        Some(_) => Placement::At,
        None => Placement::Fill,
    });
    let anchor =
        create_memo(move || OverlayAnchor::Point(area.get().map_or(Pos2::ZERO, |area| area.min)));
    let width = create_memo(move || area.get().map(|area| area.width()));
    let height = create_memo(move || area.get().map(|area| area.height()));
    let front = create_memo(move || dock.fullscreen_in_front());
    let body = dock.fullscreen.call(view! {
        <DockPanel dock tab={panel} />
    });
    let face = view! {
        <Interactive
            on_press={move |_: PointerPress| {
                if let Some(tab) = dock.fullscreen_tab() {
                    dock.show(tab);
                    dock.release_dialog_focus();
                }
            }}
        >
            <Frame @test_id="dock.fullscreen" width=width height=height>{body}</Frame>
        </Interactive>
    };
    let hosted = opens_windows();
    let overlaid = create_memo(move || open.get() && !hosted.get());
    let windowed = create_memo(move || open.get() && hosted.get());
    let in_overlay = create_memo(move || (!hosted.get()).then_some(face));
    let in_window = create_memo(move || hosted.get().then_some(face));
    let parent = use_window().map_or(WindowId::MAIN, |window| window.id);
    let shown = create_memo(move || {
        area.get()
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, dock.outer.get().size()))
    });
    let window_size = create_memo(move || shown.get().size());
    let window_placement = create_memo(move || WindowPlacement::Over {
        window: parent,
        at: shown.get().min,
    });
    let view = view! {
        <List spacing=0.0>
            <Overlay
                @node_ref={&dock.fullscreen_layer}
                anchor
                placement
                policy=LayerPolicy::Pass
                traps_focus=false
                open={overlaid}
            >
                <Portal node={in_overlay} />
            </Overlay>
            <Window
                @node_ref={&dock.fullscreen_window}
                id={None}
                size={window_size}
                placement={window_placement}
                open={windowed}
                fullscreen=true
                on_report={move |report: WindowReport| {
                    if let WindowReport::Focused { focused: true, .. } = report
                        && let Some(tab) = dock.fullscreen_tab()
                    {
                        dock.show(tab);
                    }
                }}
            >|_: WindowHandle|
                <Portal node={in_window} />
            </Window>
        </List>
    };
    create_effect(move || {
        if !windowed.get() || !untrack(|| front.get()) {
            return;
        }
        if let Some(tab) = untrack(|| dock.fullscreen_tab()) {
            untrack(|| dock.give_keyboard(tab));
        }
    });
    let layers = [dock.fullscreen_layer, dock.fullscreen_window];
    create_effect(move || {
        let shown = overlaid.get() || windowed.get();
        if !(shown && front.get()) {
            return;
        }
        for layer in layers.iter().filter_map(NodeRef::try_get) {
            untrack(|| layer.raise_overlay());
        }
    });
    view
}

#[component]
fn DockStack(dock: State) -> NodeId {
    let state = dock.state;
    let stacked = create_memo(move || state.with(DockState::stacked_tab));
    let staged = create_memo(move || {
        let tab = stacked.get()?;
        state.with(|state| state.compact_tree_of(tab))
    });
    let shown = create_memo(move || match staged.get() {
        Some(tree) => state.with(|state| state.stage_tab(tree)).or(stacked.get()),
        None => stacked.get(),
    });
    let away = create_memo(move || {
        state.with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab))
    });
    let panel = create_signal(None);
    let given = StoredCell::new(None);
    create_effect_on(
        move || {
            shown
                .get()
                .filter(|tab| away.get() != Some(*tab) && staged.with(Option::is_none))
        },
        move |&tab, previous| {
            let left = previous
                .copied()
                .flatten()
                .filter(|left| Some(*left) != tab);
            let held = left.is_some_and(|left| dock.remember_focus(left));
            let moved = tab.is_some() && dock.keyboard_after_move.get() == tab;
            if moved {
                dock.keyboard_after_move.set(None);
            }
            given.set(tab.filter(|_| held || moved));
            panel.set(tab);
        },
    );
    let vacant = create_memo(move || shown.with(Option::is_none));
    let occupied = create_memo(move || shown.with(Option::is_some));
    let backward = create_memo(move || state.with(|state| state.back().is_some()));
    let switching = create_signal(false);
    let tabs = create_memo(move || {
        state.with(|state| one_per_document(state, state.switchable(state.recent_tabs())))
    });
    let bar = dock
        .stack
        .map(|stack| stack.call(stack_handle(dock, shown, backward, tabs, switching)));
    let switcher = dock
        .switcher
        .map(|switcher| switcher.call(switcher_handle(dock, shown, tabs, switching)));
    let barred = create_memo(move || bar.is_some() && occupied.get());
    let gone = dock.away_from(shown);
    let left = create_memo(move || gone.with(Option::is_some) && staged.with(Option::is_none));
    let here = create_memo(move || {
        occupied.get() && gone.with(Option::is_none) && staged.with(Option::is_none)
    });
    give_keyboard_once_shown(dock, panel, given);
    view! {
        <BackSlide
            curves={dock.curves.slide}
            enabled={backward}
            shade={dock.back_shade}
            behind={move || {
                let dock = dock;
                view! {
                    <BackPreview dock switching />
                }
            }}
            on_back={move || dock.go_back()}
        >
            <List spacing=0.0>
                <Show value={barred}>
                    <Portal node={bar} />
                </Show>
                <Show value={vacant}>
                    <ListItem grow=1.0>
                        <EmptyPane dock leaf={state.with_untracked(vacant_leaf)} />
                    </ListItem>
                </Show>
                <Show value={here}>
                    <ListItem grow=1.0>
                        <DockBarred
                            dock
                            leaf={None}
                            holder={BarHolder::Tab { tab: shown, stacked: true }}
                        >
                            <DockPanel dock tab={panel} />
                        </DockBarred>
                    </ListItem>
                </Show>
                <Show value={left}>
                    <ListItem grow=1.0>
                        <DockAway dock gone />
                    </ListItem>
                </Show>
                <Dynamic value={staged} fallback=()>|tree: Tree| {
                    let dock = dock;
                    view! {
                        <ListItem grow=1.0>
                            <DockCompactView dock tree />
                        </ListItem>
                    }
                }</Dynamic>
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
    let tabs = one_per_document(&after, after.switchable(after.recent_tabs()));
    let tabs = create_memo(move || tabs.clone());
    let bar = dock
        .stack
        .map(|stack| stack.call(stack_handle(dock, shown, backward, tabs, switching)));
    view! {
        <List spacing=0.0>
            {bar}
            <ListItem grow=1.0>
                <Kept key={Held::Panel(behind)}>|| { dock.build_panel(behind) }</Kept>
            </ListItem>
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
    // A joined editor's stage stands in for the tab its dock took the place of: the bar names the
    // document, not the editor's stage tab. Its menu stays the stage's own, deliberately: what
    // the stage offers (its editor's actions) is what the bar holds.
    let standing = create_memo(move || shown.get().map(|tab| dock.standing(tab)));
    let title = create_memo(move || {
        standing
            .get()
            .map(|tab| dock.title(tab))
            .unwrap_or_default()
    });
    let notice = create_memo(move || standing.get().and_then(|tab| dock.notice(tab)));
    let icon = create_memo(move || standing.get().map(|tab| dock.icon(tab)).unwrap_or_default());
    let menus = dock.menus;
    let menu = create_memo(move || dock_menu_items(menus.read_only(), shown.get()));
    // The badge counts what the switcher lists as open, its cards: not the tabs that are always
    // there (the files list, settings), which it lists as rows.
    let open_tabs = create_memo(move || {
        tabs.with(|tabs| {
            tabs.iter()
                .filter(|tab| dock.standing_closable(**tab))
                .count()
        })
    });
    let switch_label = create_memo(move || match open_tabs.get() {
        0 => "No open tabs".to_owned(),
        1 => "1 open tab".to_owned(),
        count => format!("{count} open tabs"),
    });
    DockStackHandle {
        title,
        notice,
        icon,
        backward,
        menu,
        back: Callback::new(move |()| dock.go_back()),
        open_tabs,
        switch_label,
        open_switcher: Callback::new(move |()| switching.set(true)),
    }
}

fn switcher_handle(
    dock: State,
    shown: Memo<Option<TabId>>,
    tabs: Memo<Vec<TabId>>,
    switching: Signal<bool>,
) -> DockSwitcherHandle {
    let state = dock.state;
    let open = create_memo(move || {
        tabs.with(|tabs| {
            tabs.iter()
                .copied()
                .filter(|tab| dock.standing_closable(*tab))
                .collect::<Vec<_>>()
        })
    });
    let fixed = create_memo(move || {
        state.with(|state| {
            one_per_document(state, state.switchable(state.all_tabs()))
                .into_iter()
                .filter(|tab| !dock.standing_closable(*tab))
                .collect::<Vec<_>>()
        })
    });
    DockSwitcherHandle {
        open: switching.read_only(),
        close: Callback::new(move |()| switching.set(false)),
        tabs: open,
        fixed,
        // A card shows the document its tab stands in for, and picking it shows the tab itself.
        card: Func::new(move |tab: TabId| DockSwitcherCardHandle {
            tab,
            test_key: dock.test_key(dock.standing(tab)),
            title: create_memo(move || dock.title(dock.standing(tab))),
            icon: create_memo(move || dock.icon(dock.standing(tab))),
            current: create_memo(move || shown.get() == Some(tab)),
            closable: dock.standing_closable(tab),
            menu: create_memo(move || {
                closing_switcher(
                    dock_menu_items(dock.menus.read_only(), Some(tab)),
                    switching,
                )
            }),
            pick: Callback::new(move |()| {
                switching.set(false);
                dock.show(tab);
            }),
            close: Callback::new(move |()| dock.close_tab(dock.standing(tab))),
        }),
    }
}

/// `tabs` with one tab for each document they show: a joined editor's stage and the tab it stands
/// in for (`DockState::standing_in`) are one, kept at the first.
fn one_per_document(state: &DockState, tabs: Vec<TabId>) -> Vec<TabId> {
    let mut seen = Vec::new();
    tabs.into_iter()
        .filter(|tab| {
            let standing = state.standing_in(*tab);
            let new = !seen.contains(&standing);
            seen.push(standing);
            new
        })
        .collect()
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
    let windows = create_memo(move || {
        let mut windows = current.with(DockState::shown_windows);
        windows.sort();
        windows
    });
    // A carried tab or pane lands on a place the dock's layout offers (`State::spec`), and the
    // dock parts for it there.
    let tiles = NodeRef::new();
    let site = use_drop_site(
        Func::new(move |carry: DockCarry| {
            carry.dock == dock.id && !matches!(carry.dragged, DockDragged::Bar(_))
        }),
        Func::new(move |carry: DockCarry| dock.spec(carry.dragged, tiles)),
        Callback::new(move |(carry, place): (DockCarry, fluid::DockPlace)| {
            dock.commit_drop(carry.dragged, place)
        }),
    );
    dock.over.set(Some(site.over));
    // A carried tool bar lands in a slot of a band, or floats (`State::bar_spec`).
    let bars = use_drop_site(
        Func::new(move |carry: DockCarry| {
            carry.dock == dock.id && matches!(carry.dragged, DockDragged::Bar(_))
        }),
        Func::new(move |carry: DockCarry| match carry.dragged {
            DockDragged::Bar(bar) => dock.bar_spec(bar, tiles),
            _ => beui_view::drag::floating(),
        }),
        Callback::new(move |(carry, place): (DockCarry, bars::BarPlace)| {
            if let DockDragged::Bar(bar) = carry.dragged {
                dock.commit_bar(bar, place);
            }
        }),
    );
    dock.lifting.over.set(Some(bars.over));
    let built = view! {
        <List @node_ref=&tiles spacing=0.0>
            <Dynamic value={move || current.with(DockState::main)}>|surface: SurfaceId| {
                let dock = dock;
                view! {
                    <ListItem grow=1.0>
                        <DockTreeView dock tree={Tree::Surface(surface)} />
                    </ListItem>
                }
            }</Dynamic>
            <ForEach value={windows}>|surface: SurfaceId| {
                let dock = dock;
                view! {
                    <DockWindowView dock surface />
                }
            }</ForEach>
            <DockBarWindows dock />
            <DockBarLifted dock />
        </List>
    };
    // Measured from the start, so a lift reads where the tiles are at once, also on
    // a core that measures a node only once asked (`State::spec`).
    measure(built);
    built
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
    bar_shown: Memo<bool>,
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
        bar_shown,
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
            <Show value={bar_shown}>
                <ListItem length={sidebar_length(vertical, sidebar_width)}>
                    <Portal node={bar} />
                </ListItem>
            </Show>
            {sidebar_splitter.map(|splitter| view! {
                <ListItem length={dock.thickness + 2.0 * dock.reach}>{splitter}</ListItem>
            })}
            <ListItem grow=1.0>{body}</ListItem>
        </List>
    };
    let state = dock.state;
    let through = beui_dock::dock_shows_through();
    let see_through = create_memo(move || {
        (!floating && through.get())
            || leaf.is_some_and(|leaf| state.with(|state| state.shows_see_through(leaf)))
    });
    dock.chrome.call(DockChromeHandle {
        floating,
        barred,
        focused,
        see_through,
        content,
    })
}

#[component]
fn DockTreeView(dock: State, tree: Tree) -> NodeId {
    let state = dock.state;
    let mode = dock.mode;
    let staged = create_memo(move || {
        mode.get() != DockMode::Tiled && state.with(|state| state.stage_of(tree).is_some())
    });
    view! {
        <List spacing=0.0>
            <Show
                value={staged}
                fallback={move || view! {
                    <ListItem grow=1.0>
                        <DockTreeCanvas dock tree />
                    </ListItem>
                }}
            >
                <ListItem grow=1.0>
                    <DockCompactView dock tree />
                </ListItem>
            </Show>
        </List>
    }
}

#[component]
fn DockTreeCanvas(
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
    let placement_held = create_signal(None::<Measured>);
    let thickness = dock.thickness;
    let reach = dock.reach;
    let state = dock.state;
    let shape = create_memo(move || state.with(|state| tree_shape(state, tree)));
    let floats = match tree {
        Tree::Surface(surface) => {
            state.with_untracked(|state| state.window_rect(surface).is_some())
        }
        Tree::Group(_) => false,
    };
    let area = dock.rect;
    let opens = dock.opens;
    let waiting = create_memo(move || floats && !opens.get() && area.get().size() == Vec2::ZERO);
    let leaves = create_memo(move || match waiting.get() {
        true => Vec::new(),
        false => shape.with(|shape| shape.as_ref().map(DockShape::leaves).unwrap_or_default()),
    });
    let splits = create_memo(move || {
        shape.with(|shape| shape.as_ref().map(DockShape::splits).unwrap_or_default())
    });
    // While a carried tab or pane is over a place, the panes are laid out as the drop there
    // would leave them, and spring between that and where they were drawn.
    let parting = fluid::Parting::new(dock.curves.settle);
    let target = create_memo(move || {
        let after = dock.previewed()?;
        tree_shape(&after, tree)
    });
    create_effect_on_change(
        move || (shape.get(), target.get()),
        move |(shape, target), (_, was)| {
            if target.is_some() || was.is_some() {
                parting.part(shape.as_ref(), target.as_ref(), thickness, reach);
            }
        },
    );
    let arrangement = create_memo(move || {
        let shape = shape.get()?;
        let target = target.get();
        let moving = parting.moving();
        Some(CanvasArrangement::new(move |size| {
            parting.arrange(
                &shape,
                target.as_ref(),
                &moving,
                Rect::from_min_size(Pos2::ZERO, size),
                (thickness, reach),
            )
        }))
    });
    let hoisted = hoisted.get_untracked();
    let marked_surface = match tree {
        Tree::Surface(surface) => Some(surface),
        Tree::Group(_) => None,
    };
    let pane_origin = create_memo(move || {
        placement_held
            .get()
            .map_or(Pos2::ZERO, |held| held.rect.min)
    });
    let built = view! {
        <Canvas @node_ref=&canvas arrangement>
            <ForEach value={leaves}>|leaf: LeafId| {
                let dock = dock;
                let hoisted = hoisted == Some(leaf);
                view! {
                    <CanvasItem arranged={leaf_key(leaf)}>
                        <DockPanelView dock tree leaf hoisted />
                    </CanvasItem>
                }
            }</ForEach>
            <ForEach value={splits}>|split: SplitId| {
                let dock = dock;
                view! {
                    <CanvasItem arranged={split_key(split)}>
                        <DockSplitterView dock tree split />
                    </CanvasItem>
                }
            }</ForEach>
            {marked_surface.map(|surface| view! {
                <fluid::DockGhost dock={dock} surface origin={pane_origin} />
            })}
        </Canvas>
    };
    let placement_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placement_measured.get() {
            placement_held.set(Some(measured));
        }
    });
    built
}

fn leaf_key(leaf: LeafId) -> u64 {
    leaf.raw() * 2
}

fn split_key(split: SplitId) -> u64 {
    split.raw() * 2 + 1
}

fn arranged(shape: &DockShape, area: Rect, thickness: f32, reach: f32) -> Vec<(u64, Rect)> {
    let layout = shape.layout(area, thickness);
    let leaves = layout
        .leaves
        .iter()
        .map(|(leaf, rect)| (leaf_key(*leaf), *rect));
    let splits = layout.splitters.iter().map(|splitter| {
        let outset = match splitter.direction {
            Direction::Horizontal => vec2(reach, 0.0),
            Direction::Vertical => vec2(0.0, reach),
        };
        (split_key(splitter.id), splitter.handle.expand2(outset))
    });
    leaves.chain(splits).collect()
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
    let contents = create_memo(move || {
        state.with(|state| {
            state
                .entries(leaf)
                .into_iter()
                .flat_map(|entry| state.standing_tabs(entry))
                .collect::<Vec<_>>()
        })
    });
    let vacant = create_memo(move || contents.with(Vec::is_empty));
    let bar_shown =
        create_memo(move || !(vacant.get() && state.with(|state| state.hides_empty_bar(leaf))));
    let sidebar_width = dock.sidebar_width(Some(leaf));
    let icons_only = dock.icons_only(Some(leaf));
    let close = Callback::new(move |()| dock.close_leaf(leaf));
    let menu = dock.leaf_menu(Some(leaf));
    let closable = dock.all_closable(move |_| vec![leaf]);
    let claimed = modifier_claims(dock.drag_modifier);
    let resized = modifier_resize(dock, Resized::Docked);
    let shape = move || {
        let bare = state.with(|state| state.is_bare(leaf));
        let vertical = !hoisted && !bare && state.with(|state| state.is_vertical(leaf));
        (vertical, bare)
    };
    view! {
        <Interactive
            claims_touch=false
            claims={claimed}
            on_pointer={beui_components_unstyled::gestures::secondary_drag(resized)}
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
                <Dynamic value={shape}>|(vertical, bare): (bool, bool)| {
                    let dock = dock;
                    let bar = (!hoisted && !bare).then(|| DockBarHandle {
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
                        test_prefix: dock.pane_test_id("dock", leaf),
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
                        bar_shown,
                        leaf: Some(leaf),
                        body: view! {
                            <DockPanelBody dock={dock} leaf />
                        },
                    };
                    view! {
                        <ListItem grow=1.0>
                            <DockChrome dock parts />
                        </ListItem>
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
    let state = dock.state;
    let lock = create_memo(move || {
        state.with(|state| state.lock_of(leaf).map(|place| state.is_locked(&place)))
    });
    let face = dock.grip.call(DockGripHandle {
        floating: false,
        vertical,
        focused,
        locked: create_memo(move || lock.get() == Some(true)),
    });
    let menu = dock.menu;
    let title = create_memo(move || dock.dragged_title(DockDragged::Pane(leaf)));
    let icon = create_memo(move || dock.dragged_icon(DockDragged::Pane(leaf)));
    let preview = dock.preview;
    let dragged = DockDragged::Pane(leaf);
    let drag = Func::new(move |_: PointerPress| {
        Some(
            Carry::new(dock.carry(dragged))
                .preview(move || {
                    preview.call(DockPreviewHandle {
                        dragged,
                        title,
                        icon,
                        opening: false,
                        group_with: create_memo(|| None),
                    })
                })
                .outside(move |_| dock.drop_outside(dragged)),
        )
    });
    view! {
        <Interactive
            @test_id={dock.pane_test_id("dock.grip", leaf)}
            cursor=CursorIcon::Grab
            touch_drags=TouchDrags::All
            drag={Some(drag)}
        >
            <GripMenu
                menu
                vertical
                icons_only={dock.icons_only(Some(leaf))}
                pane
                lock
                toggle={move || dock.toggle_vertical(leaf)}
                toggle_icons={move || dock.toggle_icons_only(leaf)}
                toggle_lock={move || dock.toggle_lock(leaf)}
            >
                {face}
            </GripMenu>
        </Interactive>
    }
}

#[component]
fn DockPanelBody(dock: State, leaf: LeafId) -> NodeId {
    let state = dock.state;
    view! {
        <List spacing=0.0>
            <Dynamic
                value={move || state.with(|state| state.active_entry(leaf).and_then(Entry::group))}
                fallback={move || view! {
                    <ListItem grow=1.0>
                        <DockTabBody dock leaf />
                    </ListItem>
                }}
            >|group: GroupId| {
                let inset = dock.group_inset;
                view! {
                    <ListItem grow=1.0>
                        <Frame padding=inset>
                            <DockTreeView dock tree={Tree::Group(group)} />
                        </Frame>
                    </ListItem>
                }
            }</Dynamic>
        </List>
    }
}

#[component]
fn DockTabBody(dock: State, leaf: LeafId) -> NodeId {
    let state = dock.state;
    let shown = create_memo(move || state.with(|state| state.active_tab(leaf)));
    let occupied = create_memo(move || shown.with(Option::is_some));
    view! {
        <List spacing=0.0>
            <Show
                value={occupied}
                fallback={move || view! {
                    <ListItem grow=1.0>
                        <EmptyPane dock leaf />
                    </ListItem>
                }}
            >
                <ListItem grow=1.0>
                    <DockShownBody dock leaf={Some(leaf)} shown />
                </ListItem>
            </Show>
        </List>
    }
}

/// What an empty pane shows, told which pane it is (`use_dock_pane`).
#[component]
fn EmptyPane(dock: State, leaf: LeafId) -> NodeId {
    provide_context(DockPaneControl::new(
        leaf,
        Callback::new(move |leaf: LeafId| {
            dock.content_keyboard.set(None);
            if dock.state.with_untracked(|state| state.focused_leaf()) != Some(leaf) {
                dock.edit(|state| state.focus(leaf));
            }
        }),
    ));
    dock.empty.call(leaf)
}

/// What the dock's `KeepAlive` keeps while hidden: a tab's panel or a bar's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Held {
    Panel(TabId),
    Bar(TabId),
}

/// `tab`'s panel, kept built and paused while no `DockPanel` shows it.
#[component]
fn DockPanel(dock: State, tab: Prop<Option<TabId>>) -> NodeId {
    view! {
        <Dynamic value={tab} fallback=()>|tab: TabId| {
            dock.count_showing(tab);
            view! {
                <Kept key={Held::Panel(tab)}>|| { dock.build_panel(tab) }</Kept>
            }
        }</Dynamic>
    }
}

/// Gives the keyboard to the panel `given` names once `shown`'s `DockPanel` shows it: an effect
/// made after that view, so it runs after the view's control flow has put the panel in.
fn give_keyboard_once_shown(
    dock: State,
    shown: Signal<Option<TabId>>,
    given: StoredCell<Option<TabId>>,
) {
    create_effect(move || {
        shown.with(|_| ());
        let Some(tab) = given.try_get().flatten() else {
            return;
        };
        if shown.get() != Some(tab) {
            given.set(None);
            return;
        }
        // Again once the panel is built, which can be after its tab is shown.
        if !dock.panels.with(|panels| panels.contains_key(&tab)) {
            return;
        }
        given.set(None);
        untrack(|| dock.give_keyboard(tab));
    });
}

#[component]
fn DockShownBody(dock: State, leaf: Option<LeafId>, shown: Memo<Option<TabId>>) -> NodeId {
    let state = dock.state;
    let away = create_memo(move || {
        state.with(|state| state.fullscreen().map(|fullscreen| fullscreen.tab))
    });
    let panel = create_signal(None);
    let given = StoredCell::new(None);
    create_effect_on(
        move || shown.get().filter(|tab| away.get() != Some(*tab)),
        move |&tab, previous| {
            let left = previous
                .copied()
                .flatten()
                .filter(|left| Some(*left) != tab);
            let held = left.is_some_and(|left| dock.remember_focus(left));
            let moved = tab.is_some() && dock.keyboard_after_move.get() == tab;
            if moved {
                dock.keyboard_after_move.set(None);
            }
            given.set(tab.filter(|_| held || moved));
            panel.set(tab);
        },
    );
    let gone = dock.away_from(shown);
    let left = create_memo(move || gone.with(Option::is_some));
    let occupied = create_memo(move || shown.with(Option::is_some) && gone.with(Option::is_none));
    let view = view! {
        <List spacing=0.0>
            <Show value={occupied}>
                <ListItem grow=1.0>
                    <DockBarred dock leaf holder={BarHolder::Tab { tab: shown, stacked: false }}>
                        <fluid::DockHeldSize dock>
                            <DockPanel dock tab={panel} />
                        </fluid::DockHeldSize>
                    </DockBarred>
                </ListItem>
            </Show>
            <Show value={left}>
                <ListItem grow=1.0>
                    <DockAway dock gone />
                </ListItem>
            </Show>
        </List>
    };
    give_keyboard_once_shown(dock, panel, given);
    view
}

#[component]
fn DockTabBar(dock: State, leaf: LeafId, vertical: bool) -> NodeId {
    let strip = NodeRef::new();
    let tab_list = NodeRef::new();
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
    let preview = dock.preview;
    view! {
        <Reorder
            value={entries}
            spring={dock.curves.settle}
            direction
            payload={Func::new(move |entry: Entry| Some(dock.carry(DockDragged::Entry(entry))))}
            accepts={Func::new(move |carry: DockCarry| {
                carry.dock == dock.id && dock.bar_admits(leaf, carry.dragged)
            })}
            text={move |entry: Entry| dock.entry_title(entry)}
            on_move={move |moved: ReorderMove<Entry, DockCarry>| dock.move_into_bar(leaf, moved)}
            on_outside={move |(carry, _): (DockCarry, DragPoint)| dock.drop_outside(carry.dragged)}
            preview={move |carry: DockCarry| {
                let dragged = carry.dragged;
                preview.call(DockPreviewHandle {
                    dragged,
                    title: create_memo(move || dock.dragged_title(dragged)),
                    icon: create_memo(move || dock.dragged_icon(dragged)),
                    opening: false,
                    group_with: create_memo(|| None),
                })
            }}
        >|reorder: ReorderHandle<Entry, DockCarry>| {
            dock.bars.value().borrow_mut().insert(leaf, TabBar {
                strip,
                tabs: tab_list,
            });
            dock.part_for_bar(leaf, reorder.gap);
            let order = reorder.order;
            let selected = create_memo(move || state.with(|state| state.active_entry(leaf)));
            let pick = Callback::new(move |entry: Entry| {
                dock.edit(|state| {
                    let at = state.entries(leaf).iter().position(|shown| *shown == entry);
                    if let Some(at) = at {
                        state.set_active_index(leaf, at);
                    }
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
            });
            // Scrolls the tab of `entry` into view, if it is out of view; false while it has no node.
            let handles = dock.handles;
            let reveal = move |entry: Entry| {
                let tab = state.with_untracked(|state| state.entry_tabs(entry).first().copied());
                let node = handles.with_untracked(|handles| {
                    tab.and_then(|tab| handles.get(&tab))
                        .and_then(|(handle, _)| handle.try_get())
                });
                node.inspect(|node| node.reveal()).is_some()
            };
            // Another tab shown, picked from the strip's menu or by a key, scrolls into view.
            let revealed = StoredCell::new(selected.get_untracked());
            create_effect(move || {
                let Some(entry) = selected.get() else {
                    return;
                };
                // Tracked: a tab shown before its node is made is revealed once it is.
                handles.with(|_| ());
                if revealed.get() != Some(entry) && reveal(entry) {
                    revealed.set(Some(entry));
                }
            });
            let menu = create_memo(move || {
                let selected = selected.get();
                let rows = entries.get().into_iter().enumerate().map(|(index, entry)| {
                    let id = match entry {
                        Entry::Tab(tab) => format!("tab.{}", dock.test_key(tab)),
                        Entry::Group(_) => format!("group.{index}"),
                    };
                    let chosen = selected == Some(entry);
                    (id, entry, chosen)
                });
                strip_menu(dock, rows, move |entry| pick.call(entry))
            });
            let tabs = view! {
                <Choice
                    @node_ref=&tab_list
                    options={view! {
                        <ForEach value={order}>|entry: Entry| {
                            let dock = dock;
                            let label = create_memo(move || dock.entry_title(entry));
                            view! {
                                <ChoiceOption value=entry label={label} />
                            }
                        }</ForEach>
                    }}
                    selected={Binding::new(selected, move |entry: Option<Entry>| {
                        if let Some(entry) = entry {
                            pick.call(entry);
                        }
                    })}
                    item={move |item: ChoiceItem<Entry>| {
                        let ChoiceItem { value, button } = item;
                        view! {
                            <ReorderItem reorder item=value>|_: ReorderItemHandle| {
                                button
                            }</ReorderItem>
                        }
                    }}
                    handle={reorder.rows}
                    kind=ChoiceKind::Tabs
                    cursor=CursorIcon::Default
                    direction
                    shrink={create_memo(move || entries.with(Vec::len) == 1)}
                    spacing={dock.tab_spacing}
                >|handle: ChoiceOptionHandle<Entry>| {
                    let dock = dock;
                    let entry = handle.value;
                    view! {
                        <DockTabView dock leaf entry handle vertical />
                    }
                }</Choice>
            };
            let scroll = dock.strip.call(DockStripHandle {
                direction,
                tabs,
                menu,
                test_prefix: dock.pane_test_id("dock", leaf),
            });
            strip.fill(scroll);
            // Measured from the start: a lift's places leave the strip to the bar.
            let laid = measure(scroll);
            // A strip laid shorter than before (its pane narrowed) keeps its shown tab in view.
            // Not when it grows, which hides nothing, so a scroll the person made stays; nor on
            // its first layout, whose reveal would also scroll a page the dock sits in. A reveal
            // leaves a tab already in view alone.
            let length = create_memo(move || {
                laid.with(|laid| laid.map(|laid| direction.main(laid.size)))
            });
            let was = StoredCell::new(None);
            create_effect(move || {
                let length = length.get();
                if let (Some(was), Some(length)) = (was.replace(length), length)
                    && length < was
                    && let Some(entry) = selected.get_untracked()
                {
                    reveal(entry);
                }
            });
            scroll
        }</Reorder>
    }
}

#[component]
fn DockTabView(
    dock: State,
    leaf: LeafId,
    entry: Entry,
    handle: ChoiceOptionHandle<Entry>,
    vertical: bool,
) -> NodeId {
    let ChoiceOptionHandle {
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
    let dragged = create_memo(move || dock.carries(DockDragged::Entry(entry)));
    let close = Callback::new(move |()| dock.close_entry(entry));
    let float = Callback::new(move |()| dock.float_entry(entry));
    let group = Callback::new(move |()| {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.group_with_next(leaf, index);
            }
        });
    });
    let split = Callback::new(move |()| {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.split_with_next(leaf, index);
            }
        });
    });
    let ungroup = Callback::new(move |()| {
        if let Entry::Group(group) = entry {
            dock.edit(|state| state.ungroup(group));
        }
    });
    let pinned = match entry {
        Entry::Group(group) => dock.state.with_untracked(|state| state.is_pinned(group)),
        Entry::Tab(_) => false,
    };
    let standing = create_memo(move || dock.standing_tab(entry));
    let floating = dock.state.with_untracked(|state| {
        !state.is_nested(leaf)
            && state
                .surface_of(leaf)
                .and_then(|surface| state.window_rect(surface))
                .is_some()
    });
    let host = match entry {
        Entry::Group(group) => untrack(|| dock.group_host.call(group)),
        Entry::Tab(_) => None,
    };
    let anchored = create_memo(move || {
        state.with(|state| !state.admits(entry, DockDrop::Window { pos: Pos2::ZERO }))
    });
    let closable = create_memo(move || dock.entry_closable(entry));
    let waiting = create_memo(move || tabs.with(|tabs| dock.waiting(tabs)));
    let notice =
        create_memo(move || tabs.with(|tabs| tabs.iter().find_map(|tab| dock.notice(*tab))));
    let icons_only = dock.icons_only(Some(leaf));
    let grouped = matches!(entry, Entry::Group(_)) && standing.get_untracked().is_none();
    let face = dock.tab.call(DockTabHandle {
        entry,
        leaf,
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
        host,
        test_key: match entry {
            Entry::Tab(tab) => Some(dock.test_key(tab)),
            Entry::Group(_) => host.map(|host| dock.test_key(host)),
        },
        grouped,
        notice,
        close,
    });
    let menu = dock.menu;
    let fullscreen = create_memo(move || match entry {
        Entry::Tab(tab) => dock
            .offers_fullscreen(tab)
            .then(|| dock.fullscreen_tab() == Some(tab)),
        Entry::Group(_) => None,
    });
    let toggle_fullscreen = Callback::new(move |()| {
        if let Entry::Tab(tab) = entry {
            dock.toggle_fullscreen(tab);
        }
    });
    let tab_test_id = match entry {
        Entry::Tab(tab) => format!("dock.tab.{}", dock.test_key(tab)),
        Entry::Group(_) => String::new(),
    };
    let handle = NodeRef::new();
    let grouped_entry = matches!(entry, Entry::Group(_));
    let handles = dock.handles;
    create_effect(move || {
        let shown = tabs.get();
        handles.update(|handles| {
            handles.retain(|tab, (held, _)| *held != handle || shown.contains(tab));
            for tab in shown {
                let own = handles
                    .get(&tab)
                    .is_some_and(|(held, group)| *held != handle && !group);
                if !(grouped_entry && own) {
                    handles.insert(tab, (handle, grouped_entry));
                }
            }
        });
    });
    on_cleanup(move || {
        if handles.is_alive() {
            handles.update(|handles| handles.retain(|_, (held, _)| *held != handle));
        }
    });
    view! {
        <TabMenu
            @node_ref=&handle
            @test_id={tab_test_id}
            menu
            grouped
            floating
            anchored
            pinned
            has_next
            closable
            fullscreen
            close={close}
            float={float}
            group={group}
            split={split}
            ungroup={ungroup}
            toggle_fullscreen={toggle_fullscreen}
        >
            {face}
        </TabMenu>
    }
}

#[component]
fn TabMenu(
    menu: MenuStyle,
    grouped: bool,
    floating: bool,
    anchored: Memo<bool>,
    pinned: bool,
    has_next: Memo<bool>,
    closable: Memo<bool>,
    fullscreen: Memo<Option<bool>>,
    close: Callback<()>,
    float: Callback<()>,
    group: Callback<()>,
    split: Callback<()>,
    ungroup: Callback<()>,
    toggle_fullscreen: Callback<()>,
    children: NodeId,
) -> NodeId {
    let offered = create_memo(move || fullscreen.with(Option::is_some));
    let fullscreen_label = create_memo(move || match fullscreen.get() {
        Some(true) => "Leave full screen".to_owned(),
        Some(false) | None => "Full screen".to_owned(),
    });
    let stuck = create_memo(move || floating || anchored.get());
    let alone = create_memo(move || !has_next.get());
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
                <MenuItem
                    label="Pop out into a window"
                    disabled={stuck}
                    @test_id="dock.tab.pop-out"
                    on_click={move || float.call(())}
                />
                <MenuItem label={group_label} disabled={alone} on_click={move || group.call(())} />
                <MenuItem label={split_label} disabled={alone} on_click={move || split.call(())} />
                <MenuItem
                    label={close_label}
                    disabled={move || !closable.get()}
                    on_click={move || close.call(())}
                />
                {grouped.then(|| view! {
                    <MenuItem
                        label="Ungroup"
                        disabled={pinned}
                        on_click={move || ungroup.call(())}
                    />
                })}
                <Show value={offered}>
                    <MenuItem
                        label={fullscreen_label}
                        on_click={move || toggle_fullscreen.call(())}
                    />
                </Show>
            }}
        >
            <Interactive
                on_middle_click={move || {
                    if closable.get_untracked() {
                        close.call(());
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
    close: Callback<()>,
}

#[component]
fn GripMenu(
    menu: MenuStyle,
    vertical: bool,
    icons_only: Memo<bool>,
    pane: GripPane,
    lock: Memo<Option<bool>>,
    toggle: Callback<()>,
    toggle_icons: Callback<()>,
    toggle_lock: Callback<()>,
    children: NodeId,
) -> NodeId {
    let label = match vertical {
        true => "Show tabs across the top",
        false => "Show tabs in a sidebar",
    };
    let checked = create_memo(move || Some(icons_only.get()));
    let lockable = create_memo(move || lock.with(Option::is_some));
    let GripPane {
        menu: tab_menu,
        closable,
        close,
    } = pane;
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
                <MenuItem label on_click={move || toggle.call(())} />
                <MenuItem label="Icons only" checked on_click={move || toggle_icons.call(())} />
                <MenuItem
                    label="Close all tabs"
                    disabled={move || !closable.get()}
                    on_click={move || close.call(())}
                />
                <Show value={lockable}>
                    <MenuItem
                        label="Locked"
                        checked={lock}
                        @test_id="dock.pane.locked"
                        on_click={move || toggle_lock.call(())}
                    />
                </Show>
                <MenuNodes nodes={tab_menu} />
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
    let size = create_memo(move || state.with(|state| state.split_size(split)));
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Splitter);
        match size.get() {
            SplitSize::Fraction(fraction) => {
                node.set_numeric_value(f64::from(fraction * 100.0));
                node.set_max_numeric_value(100.0);
            }
            SplitSize::First(points) | SplitSize::Second(points) => {
                node.set_numeric_value(f64::from(points));
            }
        }
        node.set_min_numeric_value(0.0);
        node
    }));
    let face = dock.splitter.call(DockSplitterHandle {
        direction,
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
    });
    let carried = SplitterCarry {
        dock: dock.id,
        split,
    };
    let handle = NodeRef::new();
    // The size at the lift, written back unless the carry is dropped.
    let started = Rc::new(Cell::new(None::<SplitSize>));
    let dropped = Rc::new(Cell::new(false));
    // The splitter is carried on the board, lifted at a mouse's press: a `free` place over
    // its tree's canvas, the size the pointer moved it to, previewed by writing it.
    let site = use_drop_site(
        Func::new(move |carry: SplitterCarry| carry == carried),
        Func::new({
            let started = Rc::clone(&started);
            move |_: SplitterCarry| {
                let area = dock
                    .panes
                    .try_with(|panes| panes.borrow().get(&tree).copied())
                    .flatten();
                let space = beui_view::drag::space();
                let placed =
                    |node: Option<NodeRef>| node?.try_get().and_then(|node| space.placed(node));
                let (Some(area), Some((within, scale)), Some((at, _)), Some(splitter)) = (
                    area,
                    placed(area),
                    placed(Some(handle)),
                    dock.splitter_of(tree, split),
                ) else {
                    return beui_view::drag::floating();
                };
                // Where the splitter is in the canvas, which the item's rect is measured in.
                let home = (at.min - within.min) / scale;
                let (start, available) = (splitter.size, splitter.available(dock.thickness));
                started.set(Some(start));
                beui_view::drag::free(area, move |rect: Rect| {
                    // A press that never moves leaves the size as it is, not clamped.
                    let travel = direction.main(rect.min.to_vec2() - home);
                    match travel == 0.0 {
                        true => start,
                        false => start.moved(available, travel),
                    }
                })
                .unbounded()
                // A keyboard carry steps as the arrows do.
                .stepped(SPLIT_STEP * available)
            }
        }),
        // The drop writes the size it chose: the preview's effect may not have seen it yet.
        Callback::new({
            let dropped = Rc::clone(&dropped);
            move |(_, next): (SplitterCarry, SplitSize)| {
                dropped.set(true);
                dock.edit(|state| state.set_split_size(split, next));
            }
        }),
    );
    create_effect_on(
        move || site.over.get(),
        move |over: &Option<SplitSize>, _| {
            // Written at once, not queued after input: the board hears the pointer once input
            // is over, and the size must reach this frame's layout.
            if let Some(next) = *over {
                dock.edit(|state| state.set_split_size(split, next));
            }
        },
    );
    {
        let dropped = Rc::clone(&dropped);
        create_effect_on_change(
            move || drag_board().carries(&carried),
            move |held: &bool, _| {
                if *held {
                    return;
                }
                if let Some(begun) = started.take()
                    && !dropped.replace(false)
                {
                    dock.edit(|state| state.set_split_size(split, begun));
                }
            },
        );
    }
    let node = view! {
        <Interactive
            @node_ref=&handle
            @test_id={dock.splitter_test_id(split)}
            capture_presses=true
            focusable=true
            lifts_on_space=false
            lifts_on_press=true
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
                let Some(splitter) = dock.splitter_of(tree, split) else {
                    return false;
                };
                let available = splitter.available(dock.thickness);
                let next = splitter.size.moved(available, step * available);
                dock.edit(|state| state.set_split_size(split, next));
                true
            }}
            cursor={match direction {
                Direction::Horizontal => CursorIcon::ResizeHorizontal,
                Direction::Vertical => CursorIcon::ResizeVertical,
            }}
            touch_drags={TouchDrags::Along(direction)}
            on_hover_change={hovered}
            on_active_change={active}
            drag={move |_: PointerPress| {
                Some(
                    Carry::new(carried)
                        .drawn_by_site()
                        .scrolls_nothing()
                        .reads_outside(),
                )
            }}
        >
            <Frame>{face}</Frame>
        </Interactive>
    };
    above_embeds(node, Some(AboveEmbeds::Reaches));
    node
}

fn fitted(size: Vec2, bounds: Vec2) -> Vec2 {
    match bounds.x > 0.0 && bounds.y > 0.0 {
        true => size.min(bounds),
        false => size,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Walls {
    right: bool,
    bottom: bool,
}

impl Walls {
    fn of(edges: &[DockEdge]) -> Self {
        Self {
            right: edges.contains(&DockEdge::Right),
            bottom: edges.contains(&DockEdge::Bottom),
        }
    }
}

fn reachable_origin(rect: Rect, bounds: Vec2, bar: f32, walls: Walls) -> Pos2 {
    let keep_x = match walls.right {
        true => rect.width(),
        false => rect.width().min(WINDOW_KEEP_VISIBLE),
    };
    let keep_y = match walls.bottom {
        true => rect.height().max(bar),
        false => bar,
    };
    rect.kept_in(vec2(keep_x, keep_y), bounds)
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
    let node = view! {
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
                dock.drag_edit(move |state| state.drag_sidebar(leaf, next, moved));
            }}
            children={face}
        />
    };
    above_embeds(node, Some(AboveEmbeds::Reaches));
    node
}

#[component]
fn DockWindowView(dock: State, surface: SurfaceId) -> NodeId {
    let frame = NodeRef::new();
    dock.windows.value().borrow_mut().insert(surface, frame);
    let held = holds_focus(frame);
    let focused = dock.focused_windows;
    create_effect(move || {
        let held = held.get();
        untrack(|| {
            if focused.with(|windows| windows.contains(&surface)) != held {
                focused.update(|windows| match held {
                    true => windows.push(surface),
                    false => windows.retain(|window| *window != surface),
                });
            }
        });
    });
    on_cleanup(move || {
        dock.windows
            .try_with(|windows| windows.borrow_mut().remove(&surface));
        if focused.is_alive() && focused.with_untracked(|windows| windows.contains(&surface)) {
            focused.update(|windows| windows.retain(|window| *window != surface));
        }
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
    let walls = create_memo(move || state.with(|state| Walls::of(&state.dock_band_edges())));
    let placed = create_memo(move || {
        if hosted.get() {
            return rect.get();
        }
        let bounds = area.get().size();
        let rect = Rect::from_min_size(rect.get().min, fitted(rect.get().size(), bounds));
        let origin = reachable_origin(rect, bounds, reach(), walls.get());
        Rect::from_min_size(origin, rect.size())
    });
    let outset = vec2(HANDLE_REACH, HANDLE_REACH);
    let origin = create_memo(move || match hosted.get() {
        true => pos2(HANDLE_REACH, HANDLE_REACH),
        false => area.get().min + placed.get().min.to_vec2() + overshoot.get(),
    });
    let band = store(RefCell::new(Band::new(dock.curves.pull)));
    let bounce = create_animation(
        move |frame| {
            let band = band.value();
            let mut band = band.borrow_mut();
            let window = placed.get_untracked();
            let bounds = area.get_untracked().size();
            let reach = reach();
            let walls = walls.get_untracked();
            let limit = |origin: Pos2| {
                reachable_origin(
                    Rect::from_min_size(origin, window.size()),
                    bounds,
                    reach,
                    walls,
                )
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
    let window_notice = create_memo(move || {
        state
            .with(|state| {
                state
                    .leaves(surface)
                    .first()
                    .and_then(|leaf| state.active_tab(*leaf))
            })
            .and_then(|tab| dock.notice(tab))
    });
    let parent = dock.watch_home();
    let placement = create_memo(move || dock.placement(surface, parent.get()));
    let shown_at = StoredCell::new(None::<Pos2>);
    create_effect_on(
        move || area.get().min,
        move |&corner, was| {
            let Some(&was) = was else {
                return;
            };
            let Some(shown) = shown_at.get() else {
                return;
            };
            if was == corner || !hosted.get() {
                return;
            }
            let kept = was - corner;
            shown_at.set(Some(shown + kept));
            dock.edit(|state| {
                if let Some(rect) = state.window_rect(surface) {
                    state.set_window_rect(surface, rect.translate(kept));
                }
            });
        },
    );
    let contents = create_memo(move || {
        state.with(|state| {
            state
                .leaves(surface)
                .into_iter()
                .flat_map(|leaf| state.entries(leaf))
                .flat_map(|entry| state.standing_tabs(entry))
                .collect::<Vec<_>>()
        })
    });
    let vacant = create_memo(move || contents.with(Vec::is_empty));
    let close = Callback::new(move |()| {
        for leaf in dock.state.get_untracked().leaves(surface) {
            dock.close_leaf(leaf);
        }
    });
    let hoisted = state.with_untracked(|state| state.leaves(surface).first().copied());
    let sidebar_width = dock.sidebar_width(hoisted);
    let icons_only = dock.icons_only(hoisted);
    let menu = dock.leaf_menu(hoisted);
    let closable = dock.all_closable(move |state| state.leaves(surface));
    let overlay = NodeRef::new();
    dock.stacked.value().borrow_mut().insert(surface, overlay);
    on_cleanup(move || {
        dock.stacked
            .try_with(|stacked| stacked.borrow_mut().remove(&surface));
    });
    create_effect(move || {
        dock.state.track();
        if focused.get() {
            untrack(|| dock.restack(surface));
        }
    });
    let claimed = modifier_claims(dock.drag_modifier);
    let view = view! {
        <Window
            @node_ref=&overlay
            size={size}
            at={at}
            placement={placement}
            title={window_title}
            notice={window_notice}
            on_report={move |report: WindowReport| {
                if let WindowReport::Moved { origin, .. } = report
                    && let Some(home) = window_origin(parent.get_untracked())
                {
                    let min = origin - home.to_vec2() + outset - area.get_untracked().min.to_vec2();
                    shown_at.set(Some(min));
                    dock.edit(|state| {
                        if let Some(rect) = state.window_rect(surface) {
                            state.set_window_rect(surface, Rect::from_min_size(min, rect.size()));
                        }
                    });
                }
                dock.window_reported(surface, report);
            }}
        >|handle: WindowHandle| {
            create_effect(move || {
                let min = rect.get().min;
                if !hosted.get() || shown_at.get().is_none_or(|shown| shown == min) {
                    return;
                }
                shown_at.set(Some(min));
                handle.place(WindowPlacement::Over {
                    window: parent.get_untracked(),
                    at: area.get_untracked().min + min.to_vec2() - outset,
                });
            });
            let grabbed = StoredCell::new(None::<(Rect, Pos2)>);
            let resized = Callback::new(modifier_resize(dock, Resized::Window(surface, handle)));
            let catcher = NodeRef::new();
            let window_bar = create_signal(Area::Nowhere);
            create_effect(move || {
                let bar = catcher.try_get().and_then(|catcher| {
                    let origin = tracked_rect(catcher)?.min.to_vec2();
                    Some(dock.window_bar(surface)?.translate(-origin))
                });
                window_bar.set(bar.map_or(Area::Nowhere, Area::Within));
            });
            view! {
                <Root clear_color=Color::TRANSPARENT>
                    <Frame width=outer_width height=outer_height>
                        <Canvas>
                            <CanvasItem x=HANDLE_REACH y=HANDLE_REACH width={width} height={height}>
                                <Frame @node_ref=&frame width=width height=height>
                                    <Canvas>
                                        <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                                            <Interactive
                                                @node_ref=&catcher
                                                capture_presses={window_bar}
                                                claims={claimed}
                                                on_pointer={beui_components_unstyled::gestures::secondary_drag(resized)}
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
                                                    let origin = reachable_origin(placed, bounds, reach(), walls.get_untracked());
                                                    let banding = motion().animates();
                                                    let band = band.value();
                                                    let mut band = band.borrow_mut();
                                                    band.stretch(origin, placed.min - origin, bounds, banding, beui_view::reactive::now());
                                                    overshoot.set(band.offset);
                                                    dock.drag_edit(move |state| {
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
                                                    band.value().borrow_mut().release(beui_view::reactive::now());
                                                    bounce.start();
                                                }}
                                            >
                                                <List spacing=0.0>
                                                    <Dynamic
                                                        value={move || {
                                                            hoisted.is_some_and(|leaf| state.with(|state| state.is_vertical(leaf)))
                                                        }}
                                                    >|vertical: bool| {
                                                        let dock = dock;
                                                        let grip_face = dock.grip.call(DockGripHandle {
                                                            floating: true,
                                                            vertical,
                                                            focused,
                                                            locked: create_memo(|| false),
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
                                                                lock={create_memo(|| None)}
                                                                toggle_icons={move || {
                                                                    if let Some(leaf) = hoisted {
                                                                        dock.toggle_icons_only(leaf);
                                                                    }
                                                                }}
                                                                toggle_lock={|| {}}
                                                            >
                                                                {grip_face}
                                                            </GripMenu>
                                                        };
                                                        let grip_id = hoisted.map_or_else(
                                                            || "dock.window.grip".to_owned(),
                                                            |leaf| dock.pane_test_id("dock.window.grip", leaf),
                                                        );
                                                        let grip = view! {
                                                            <Interactive
                                                                @node_ref=&grip_ref
                                                                @test_id={grip_id}
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
                                                            <DockTreeCanvas
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
                                                                test_prefix: hoisted.map_or_else(
                                                                    || "dock".to_owned(),
                                                                    |leaf| dock.pane_test_id("dock", leaf),
                                                                ),
                                                            }),
                                                            bar_shown: create_memo(|| true),
                                                            leaf: hoisted,
                                                            body: pane,
                                                        };
                                                        view! {
                                                            <ListItem grow=1.0>
                                                                <DockChrome dock parts />
                                                            </ListItem>
                                                        }
                                                    }</Dynamic>
                                                </List>
                                            </Interactive>
                                        </CanvasItem>
                                    </Canvas>
                                </Frame>
                            </CanvasItem>
                            <ForEach value={GRIPS.to_vec()}>|(grip, upright): (Grip, bool)| {
                                let dock = dock;
                                let handle_rect = create_memo(move || grip.rect(upright, size.get(), dock.window_border));
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
                                                dock.drag_edit(move |state| state.set_window_rect(surface, resized));
                                            }}
                                        ></Interactive>
                                    </CanvasItem>
                                }
                            }</ForEach>
                        </Canvas>
                    </Frame>
                </Root>
            }
            .child()
        }</Window>
    };
    create_effect(move || untrack(|| dock.restack(surface)));
    view
}

fn grip_reach(frame: &NodeRef, grip: &NodeRef) -> f32 {
    let (Some(frame), Some(grip)) = (frame.try_get(), grip.try_get()) else {
        return 0.0;
    };
    let reach = || {
        Some(
            measure(grip).get_untracked()?.rect.bottom()
                - measure(frame).get_untracked()?.rect.top(),
        )
    };
    reach().unwrap_or(0.0)
}

#[derive(Clone, Copy)]
pub struct DockApi(State);

impl DockApi {
    pub fn state(&self) -> DockState {
        self.0.state.get()
    }

    pub(crate) fn ask_bar(&self, request: DockRequest) {
        if self.0.panels.is_alive() {
            self.0.ask_bar(request);
        }
    }

    pub(crate) fn focused_dialog(&self) -> Option<TabId> {
        let focus = self.0.dialog_focus;
        focus.is_alive().then(|| focus.get()).flatten()
    }
}

fn tracked_rect(node: NodeId) -> Option<Rect> {
    Some(measure(node).get()?.rect)
}

fn modifier_claims(modifiers: Memo<Option<Modifiers>>) -> Memo<Vec<Claim>> {
    create_memo(move || {
        modifiers
            .get()
            .filter(|held| held.any())
            .map(|held| vec![Claim::holding(held, true)])
            .unwrap_or_default()
    })
}

/// How the dock's motions look: the easing gaps open and close and a sidebar
/// changes width on (`ease`), the spring a dropped bar and reordered tabs land on
/// (`settle`), the
/// spring a floating window pulled past the edge comes back on (`pull`), the
/// spring the default tab strip's tabs come back on when scrolled past an end
/// (`rubber`), and how a pane slides away on back (`slide`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DockCurves {
    pub ease: Easing,
    pub settle: Spring,
    pub pull: Spring,
    /// For the tab strip drawn when no `strip` is given.
    pub rubber: Spring,
    pub slide: SlideCurves,
}

/// The chords of the dock's Next tab and Previous tab. `CTRL` (Ctrl+Tab, Ctrl+Shift+Tab) is the
/// default; a desktop, whose programs must keep Ctrl+Tab, binds `SUPER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockTabKeys {
    pub next: Chord,
    pub previous: Chord,
}

impl DockTabKeys {
    pub const CTRL: Self = Self {
        next: Chord::ctrl(Key::Tab),
        previous: Chord::ctrl(Key::Tab).shift(),
    };
    pub const SUPER: Self = Self {
        next: Chord::logo(Key::Tab),
        previous: Chord::logo(Key::Tab).shift(),
    };
}

impl Default for DockTabKeys {
    fn default() -> Self {
        Self::CTRL
    }
}

/// The dock's tab commands as actions a palette lists. Next tab and Previous tab answer at the
/// window, before the focused control (`.capture()`), so they work from a text input in a pane
/// and, as intercepted keys, over an embedded editor's view; but only while the focus is in this
/// dock: in its area, in one of its floating windows, or on no node of this document, where keys
/// reach it only as intercepted keys from the content of its tabs (an app's view, a program's
/// window). A modal outside the dock keeps them from switching the tab under it, and of several
/// docks the one holding the focus answers.
fn tab_actions(dock: State, keys: Prop<DockTabKeys>, area: NodeRef) {
    let in_area = holds_focus(area);
    let held = focus_held();
    let windows = dock.focused_windows;
    let focused_here = create_memo(move || {
        !held.get() || in_area.get() || windows.with(|windows| !windows.is_empty())
    });
    let several = create_memo(move || {
        dock.state.with(|state| {
            state
                .focused_leaf()
                .is_some_and(|leaf| state.entries(state.standing_leaf(leaf)).len() > 1)
        })
    });
    let cycles = create_memo(move || several.get() && focused_here.get());
    // Close tab closes the entry the focused pane shows, so it is enabled by that entry's tabs.
    let closable = create_memo(move || {
        dock.shown_entry()
            .is_some_and(|(_, entry)| dock.entry_closable(entry))
    });
    // Bound again whenever the owner changes the chords: the effect's scope, and with it the
    // actions, is dropped on every rerun.
    create_effect(move || {
        let keys = keys.get();
        untrack(|| {
            Action::new("dock.tab.next", "Next tab", move || dock.cycle(false))
                .shortcut(keys.next)
                .enabled(cycles)
                .at(KeyAt::Window)
                .capture()
                .register();
            Action::new("dock.tab.previous", "Previous tab", move || {
                dock.cycle(true)
            })
            .shortcut(keys.previous)
            .enabled(cycles)
            .at(KeyAt::Window)
            .capture()
            .register();
        });
    });
    Action::new("dock.tab.split-right", "Split right", move || {
        dock.split_shown(Side::Right)
    })
    .enabled(several)
    .register();
    Action::new("dock.tab.split-down", "Split down", move || {
        dock.split_shown(Side::Below)
    })
    .enabled(several)
    .register();
    Action::new("dock.tab.close", "Close tab", move || {
        if let Some((_, entry)) = untrack(|| dock.shown_entry()) {
            dock.close_entry(entry);
        }
    })
    .enabled(closable)
    .register();
}

/// A strip's menu of its tabs: a row for each `(id, entry, shown)` that calls `pick`.
fn strip_menu(
    dock: State,
    rows: impl Iterator<Item = (String, Entry, bool)>,
    pick: impl Fn(Entry) + Copy + 'static,
) -> Vec<MenuNode> {
    rows.map(|(id, entry, shown)| {
        let action = Action::new(id, dock.entry_title(entry), move || pick(entry))
            .glyph(dock.entry_icon(entry))
            .radio(shown)
            .detached();
        MenuNode::action(&action)
    })
    .collect()
}

/// The tab `leaf` shows: its active tab, or for a joined editor's group, the tab its stage shows.
fn shown_tab(state: &DockState, leaf: LeafId) -> Option<TabId> {
    entry_shown_tab(state, state.active_entry(leaf)?)
}

/// The tab `entry` shows when it is shown: itself, or for a group, the tab its stage shows.
pub(super) fn entry_shown_tab(state: &DockState, entry: Entry) -> Option<TabId> {
    let mut entry = entry;
    for _ in 0..16 {
        match entry {
            Entry::Tab(tab) => return Some(tab),
            Entry::Group(group) => {
                let tree = Tree::Group(group);
                let leaf = match state.tree_leaves(tree)[..] {
                    [only] => only,
                    _ => state.stage_of(tree)?,
                };
                entry = state.active_entry(leaf)?;
            }
        }
    }
    None
}
