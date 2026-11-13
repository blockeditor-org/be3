use be_lib_stable_hash::HashMap;
use std::cell::Cell;
use std::rc::Rc;

use beui_dock_state::{DockAlign, DockBarPlacement, DockEdge, LeafId, TabId};
use beui_view::actions::{Action, MenuNode};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::window::WindowId;
use beui_view::types::{Direction, HostTransform, PointerPress, Pos2, Rect, Vec2};

use crate::link::{DockBarMove, DockDragStage, DockHosted, DockRequest};

pub trait DockTabDriver {
    fn handle(&self, tab: TabId) -> Option<NodeId>;
    fn fullscreen(&self, tab: TabId) -> bool;
    fn enter_fullscreen(&self, tab: TabId, area: Option<Rect>);
    fn leave_fullscreen(&self, tab: TabId);
    fn offer_fullscreen(&self, tab: TabId);
    fn focused(&self, tab: TabId) -> bool;
    fn attention(&self, tab: TabId) -> Option<u64>;
    fn show(&self, tab: TabId);
    fn content_focused(&self, tab: TabId);
    fn show_notice(&self, tab: TabId, notice: Memo<Option<String>>);
    fn window_size(&self, tab: TabId) -> Option<Vec2>;
    fn resize_window(&self, tab: TabId, size: Vec2);
    fn brought_forward(&self, tab: TabId) -> Option<u64>;
    fn keyboard(&self, tab: TabId) -> Option<u64>;
}

#[derive(Clone, Copy)]
pub struct DockTabControl {
    tab: TabId,
    backing: Backing,
}

#[derive(Clone, Copy)]
enum Backing {
    Local(Stored<Rc<dyn DockTabDriver>>),
    Hosted {
        hosted: Memo<Option<DockHosted>>,
        request: Callback<DockRequest>,
        claimed: Stored<Cell<bool>>,
    },
}

impl DockTabControl {
    pub fn local(tab: TabId, driver: Rc<dyn DockTabDriver>) -> Self {
        Self {
            tab,
            backing: Backing::Local(store(driver)),
        }
    }

    pub fn hosted(
        tab: TabId,
        hosted: Memo<Option<DockHosted>>,
        request: Callback<DockRequest>,
    ) -> Self {
        Self {
            tab,
            backing: Backing::Hosted {
                hosted,
                request,
                claimed: store(Cell::new(false)),
            },
        }
    }

    pub fn is_hosted(&self) -> bool {
        matches!(self.backing, Backing::Hosted { .. })
    }

    /// Lets one child in a hosted body claim the tab; later callers get false.
    pub fn claim_body(&self) -> bool {
        let Backing::Hosted { claimed, .. } = self.backing else {
            return false;
        };
        if claimed.with(|claimed| claimed.replace(true)) {
            return false;
        }
        on_cleanup(move || {
            claimed.try_with(|claimed| claimed.set(false));
        });
        true
    }

    fn driver(&self) -> Option<Rc<dyn DockTabDriver>> {
        match self.backing {
            Backing::Local(driver) => driver.try_value().map(|driver| Rc::clone(&*driver)),
            Backing::Hosted { .. } => None,
        }
    }

    fn read_hosted<R>(&self, read: impl FnOnce(&DockHosted) -> R) -> Option<R> {
        match self.backing {
            Backing::Hosted { hosted, .. } => hosted.with(|hosted| hosted.as_ref().map(read)),
            Backing::Local(_) => None,
        }
    }

    fn ask(&self, request: DockRequest) {
        if let Backing::Hosted { request: send, .. } = self.backing {
            send.call(request);
        }
    }

    pub fn tab(&self) -> TabId {
        self.tab
    }

    pub fn handle(&self) -> Option<NodeId> {
        self.driver()?.handle(self.tab)
    }

    pub fn anchor(&self) -> Option<Rect> {
        if self.fullscreen() {
            return None;
        }
        measure(self.handle()?).get().map(|measured| measured.rect)
    }

    pub fn fullscreen(&self) -> bool {
        if let Some(fullscreen) = self.read_hosted(|hosted| hosted.fullscreen == Some(self.tab)) {
            return fullscreen;
        }
        self.driver()
            .is_some_and(|driver| driver.fullscreen(self.tab))
    }

    pub fn enter_fullscreen(&self, area: Option<Rect>) {
        self.ask(DockRequest::Fullscreen(Some((self.tab, area))));
        if let Some(driver) = self.driver() {
            driver.enter_fullscreen(self.tab, area);
        }
    }

    pub fn leave_fullscreen(&self) {
        if self.is_hosted() && self.fullscreen() {
            self.ask(DockRequest::Fullscreen(None));
        }
        if let Some(driver) = self.driver() {
            driver.leave_fullscreen(self.tab);
        }
    }

    /// Adds Full screen / Leave full screen to the tab's right-click menu. The dock binds no key to
    /// leave fullscreen, so content that enters it some other way offers its own way out.
    pub fn offer_fullscreen(&self) {
        if let Some(driver) = self.driver() {
            driver.offer_fullscreen(self.tab);
        }
    }

    pub fn focused(&self) -> bool {
        if let Some(focused) = self.read_hosted(|hosted| hosted.focused == Some(self.tab)) {
            return focused;
        }
        self.driver().is_some_and(|driver| driver.focused(self.tab))
    }

    /// Changes each time a press on the parent's panel flashes this modal dialog, so content whose
    /// keyboard lives in another document can take it there.
    pub fn attention(&self) -> Option<u64> {
        self.driver()?.attention(self.tab)
    }

    pub fn show(&self) {
        self.ask(DockRequest::Show(self.tab));
        if let Some(driver) = self.driver() {
            driver.show(self.tab);
        }
    }

    /// The keyboard went into a view of the tab's content that this document's focus does not
    /// see (a child editor's): shows the tab, and lets the dock hand the keyboard on to the tab
    /// shown in its place, as it does for focus inside the panel.
    pub fn content_focused(&self) {
        self.ask(DockRequest::Show(self.tab));
        if let Some(driver) = self.driver() {
            driver.content_focused(self.tab);
        }
    }

    /// While `Some`, the tab, the dialog's header and the stacked bar show the notice in place of the
    /// title. Kept until the calling scope ends.
    pub fn show_notice(&self, notice: Memo<Option<String>>) {
        if let Some(driver) = self.driver() {
            driver.show_notice(self.tab, notice);
        }
    }

    pub fn window_size(&self) -> Option<Vec2> {
        self.driver()?.window_size(self.tab)
    }

    /// Sizes the floating window showing this tab, never below `MIN_WINDOW_SIZE.x` wide. Where a host
    /// opens windows this becomes a `WindowRequest::Resize`.
    pub fn resize_window(&self, size: Vec2) {
        if let Some(driver) = self.driver() {
            driver.resize_window(self.tab, size);
        }
    }

    /// Changes each time this tab's floating window comes to the front (opened, shown, switched to, or
    /// left in front by a close; not while the dock is built or restored), without moving the focus.
    pub fn brought_forward(&self) -> Option<u64> {
        self.driver()?.brought_forward(self.tab)
    }

    /// Changes each time the dock gives this tab the keyboard, so content whose keyboard lives in
    /// another document (a program's window, a child block) can ask for it there.
    pub fn keyboard(&self) -> Option<u64> {
        self.driver()?.keyboard(self.tab)
    }
}

pub fn use_dock_tab() -> Option<DockTabControl> {
    use_context::<DockTabControl>()
}

/// The pane an empty pane's view stands in, as `DockTabControl` is a tab body's tab: content that
/// takes the focus in another document (a child block) focuses the pane with it, which brings the
/// pane's window in front.
#[derive(Clone, Copy)]
pub struct DockPaneControl {
    leaf: LeafId,
    focus: Callback<LeafId>,
}

impl DockPaneControl {
    pub fn new(leaf: LeafId, focus: Callback<LeafId>) -> Self {
        Self { leaf, focus }
    }

    pub fn focus(&self) {
        self.focus.call(self.leaf);
    }
}

pub fn use_dock_pane() -> Option<DockPaneControl> {
    use_context::<DockPaneControl>()
}

#[derive(Clone, Copy)]
struct ShowsThrough(Memo<bool>);

/// Makes every dock built below show through like a `see_through` tab while `shows` is true, except
/// in floating windows; be-sdk-beui's frame uses it for an editor kept to its card on its parent's
/// stage.
pub fn provide_dock_shows_through(shows: Memo<bool>) {
    provide_context(ShowsThrough(shows));
}

pub fn dock_shows_through() -> Memo<bool> {
    match use_context::<ShowsThrough>() {
        Some(ShowsThrough(shows)) => shows,
        None => create_memo(|| false),
    }
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

/// Hands the enclosing tab's menu to the dock: shown behind More, with `.pin(..)` actions moved into
/// the bar (`Pin::Title` becomes the stacked bar's title button).
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

pub trait DockBarDriver {
    fn placement(&self, bar: TabId) -> Option<DockBarPlacement>;
    fn is_hidden(&self, bar: TabId) -> bool;
    fn set_hidden(&self, bar: TabId, hidden: bool);
    fn moving(&self, bar: TabId) -> bool;
    fn place(&self, bar: TabId, placement: DockBarPlacement);
    fn pop_out(&self, bar: TabId);
    fn begin_move(&self, bar: TabId, press: PointerPress);
}

#[derive(Clone, Copy)]
pub struct DockBarControl {
    bar: TabId,
    backing: BarBacking,
    menu: Memo<Vec<MenuNode>>,
    surface: NodeRef,
}

#[derive(Clone, Copy)]
enum BarBacking {
    Local(Stored<Rc<dyn DockBarDriver>>),
    Hosted {
        hosted: Memo<Option<DockHosted>>,
        request: Callback<DockRequest>,
        last: Signal<Option<Carry>>,
    },
}

#[derive(Clone, Copy, PartialEq)]
struct Carry {
    home: WindowId,
    start: Option<HostTransform>,
    at: Pos2,
}

impl DockBarControl {
    pub fn local(bar: TabId, driver: Rc<dyn DockBarDriver>, menu: Memo<Vec<MenuNode>>) -> Self {
        Self {
            bar,
            backing: BarBacking::Local(store(driver)),
            menu,
            surface: NodeRef::new(),
        }
    }

    pub fn hosted(
        bar: TabId,
        hosted: Memo<Option<DockHosted>>,
        request: Callback<DockRequest>,
    ) -> Self {
        let placement =
            create_memo(move || hosted.with(|hosted| hosted.as_ref()?.state.bar_placement(bar)));
        let place = Callback::new(move |placement: Option<DockBarPlacement>| {
            request.call(DockRequest::PlaceBar(bar, placement));
        });
        let hide = Callback::new(move |()| request.call(DockRequest::HideBar(bar, true)));
        Self {
            bar,
            backing: BarBacking::Hosted {
                hosted,
                request,
                last: create_signal(None),
            },
            menu: bar_menu(placement, place, hide, None),
            surface: NodeRef::new(),
        }
    }

    fn driver(&self) -> Option<Rc<dyn DockBarDriver>> {
        match self.backing {
            BarBacking::Local(driver) => driver.try_value().map(|driver| Rc::clone(&*driver)),
            BarBacking::Hosted { .. } => None,
        }
    }

    pub fn bar(&self) -> TabId {
        self.bar
    }

    pub fn surface(&self) -> NodeRef {
        self.surface
    }

    pub fn is_hosted(&self) -> bool {
        matches!(self.backing, BarBacking::Hosted { .. })
    }

    pub fn placement(&self) -> DockBarPlacement {
        let bar = self.bar;
        let placement = match self.backing {
            BarBacking::Local(_) => self.driver().and_then(|driver| driver.placement(bar)),
            BarBacking::Hosted { hosted, .. } => {
                hosted.with(|hosted| hosted.as_ref()?.state.bar_placement(bar))
            }
        };
        placement.unwrap_or(DockBarPlacement::Over(DockEdge::Top, DockAlign::Center))
    }

    pub fn edge(&self) -> DockEdge {
        self.placement().edge()
    }

    pub fn is_hidden(&self) -> bool {
        let bar = self.bar;
        match self.backing {
            BarBacking::Local(_) => self.driver().is_some_and(|driver| driver.is_hidden(bar)),
            BarBacking::Hosted { hosted, .. } => hosted.with(|hosted| {
                hosted
                    .as_ref()
                    .is_some_and(|hosted| hosted.state.is_bar_hidden(bar))
            }),
        }
    }

    pub fn hide(&self) {
        self.set_hidden(true);
    }

    pub fn show(&self) {
        self.set_hidden(false);
    }

    fn set_hidden(&self, hidden: bool) {
        match self.backing {
            BarBacking::Local(_) => {
                if let Some(driver) = self.driver() {
                    driver.set_hidden(self.bar, hidden);
                }
            }
            BarBacking::Hosted { request, .. } => {
                request.call(DockRequest::HideBar(self.bar, hidden));
            }
        }
    }

    pub fn direction(&self) -> Direction {
        self.placement().direction()
    }

    pub fn menu(&self) -> Memo<Vec<MenuNode>> {
        self.menu
    }

    pub fn moving(&self) -> bool {
        match self.backing {
            BarBacking::Local(_) => self.driver().is_some_and(|driver| driver.moving(self.bar)),
            BarBacking::Hosted { last, .. } => last.with(Option::is_some),
        }
    }

    pub fn place(&self, placement: DockBarPlacement) {
        match self.backing {
            BarBacking::Local(_) => {
                if let Some(driver) = self.driver() {
                    driver.place(self.bar, placement);
                }
            }
            BarBacking::Hosted { request, .. } => {
                request.call(DockRequest::PlaceBar(self.bar, Some(placement)));
            }
        }
    }

    pub fn pop_out(&self) {
        match self.backing {
            BarBacking::Local(_) => {
                if let Some(driver) = self.driver() {
                    driver.pop_out(self.bar);
                }
            }
            BarBacking::Hosted { request, .. } => {
                request.call(DockRequest::PlaceBar(self.bar, None));
            }
        }
    }

    /// Called from a handle's `on_drag` once past the drag threshold, with the press that went down, then
    /// with each move. A hosted control turns it into `DockRequest::MoveBar`, mapping the press back
    /// into the body's window with `window_point` when the pointer is over another window of the program.
    pub fn begin_move(&self, press: PointerPress) {
        let BarBacking::Hosted { request, last, .. } = self.backing else {
            if let Some(driver) = self.driver() {
                driver.begin_move(self.bar, press);
            }
            return;
        };
        let carried = last.get_untracked();
        let home = carried.map_or(press.window, |carry| carry.home);
        let start = carried.map_or_else(|| to_host(home), |carry| carry.start);
        let Some(at) = window_point(press.pos, press.window, home) else {
            return;
        };
        let at = match (start, to_host(home)) {
            (Some(start), Some(now)) => start.inverse_point(now.point(at)),
            _ => at,
        };
        last.set(Some(Carry { home, start, at }));
        request.call(DockRequest::MoveBar(
            self.bar,
            DockBarMove {
                at,
                stage: DockDragStage::Move,
            },
        ));
    }

    /// Drops a bar a hosted control is carrying; a local control ignores it.
    pub fn end_move(&self) {
        let BarBacking::Hosted { request, last, .. } = self.backing else {
            return;
        };
        let Some(Carry { at, .. }) = last.get_untracked() else {
            return;
        };
        last.set(None);
        request.call(DockRequest::MoveBar(
            self.bar,
            DockBarMove {
                at,
                stage: DockDragStage::Drop,
            },
        ));
    }
}

pub fn use_dock_bar() -> Option<DockBarControl> {
    use_context::<DockBarControl>()
}

/// The grip menu's "Auto-hide": shown while `offered`, checked while `on`.
#[derive(Clone, Copy)]
pub struct BarAutoHide {
    pub offered: Memo<bool>,
    pub on: Memo<bool>,
    pub set: Callback<bool>,
}

pub fn bar_menu(
    placement: Memo<Option<DockBarPlacement>>,
    place: Callback<Option<DockBarPlacement>>,
    hide: Callback<()>,
    auto_hide: Option<BarAutoHide>,
) -> Memo<Vec<MenuNode>> {
    let item = move |target: DockBarPlacement, label: String| {
        let id = format!("dock.bar.{}", target.name());
        let chosen = create_memo(move || {
            placement
                .get()
                .is_some_and(|placement| placement.same_place(target))
        });
        let action = Action::new(id, label, move || match target {
            DockBarPlacement::Window { .. } => place.call(None),
            target => place.call(Some(target)),
        })
        .radio(chosen)
        .detached();
        MenuNode::action(&action)
    };
    let mut nodes = Vec::new();
    let groups: [(
        fn(DockEdge, DockAlign) -> DockBarPlacement,
        &str,
        &str,
        &str,
        &str,
    ); 2] = [
        (
            DockBarPlacement::Over,
            "Float over the",
            "Over the",
            "dock.bar.over.ends",
            "Float at an end of an edge",
        ),
        (
            DockBarPlacement::Beside,
            "Dock along the",
            "Along the",
            "dock.bar.beside.ends",
            "Dock at an end of an edge",
        ),
    ];
    for (placed, centred, at_end, ends_id, ends_label) in groups {
        let mut ends = Vec::new();
        for edge in DockEdge::ALL {
            nodes.push(item(
                placed(edge, DockAlign::Center),
                format!("{centred} {}", edge.name()),
            ));
            for align in [DockAlign::Start, DockAlign::End] {
                if let Some(side) = align.side(edge) {
                    let label = format!("{at_end} {}, at the {side}", edge.name());
                    ends.push(item(placed(edge, align), label));
                }
            }
        }
        nodes.push(MenuNode::submenu(ends_id, ends_label, None, ends));
        nodes.push(MenuNode::Separator);
    }
    nodes.push(item(
        DockBarPlacement::Window {
            edge: DockEdge::Top,
            origin: Pos2::ZERO,
        },
        "Pop out into a window".to_owned(),
    ));
    nodes.push(MenuNode::Separator);
    let auto_hidden = auto_hide.map(|auto_hide| {
        let action = Action::new("dock.bar.auto-hide", "Auto-hide", move || {
            auto_hide.set.call(!untrack(|| auto_hide.on.get()))
        })
        .checked(auto_hide.on)
        .detached();
        (auto_hide.offered, MenuNode::action(&action))
    });
    let hidden = Action::new("dock.bar.hide", "Hide", move || hide.call(())).detached();
    nodes.push(MenuNode::action(&hidden));
    create_memo(move || {
        let mut shown = nodes.clone();
        if let Some((offered, item)) = &auto_hidden
            && offered.get()
        {
            shown.insert(shown.len() - 1, item.clone());
        }
        shown
    })
}
