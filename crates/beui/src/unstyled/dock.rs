mod state;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayAnchor, OverlayMode, Placement};
use crate::base::{Direction, ItemSize};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Canvas, CanvasItem, ClickCallback, ClickCatcher, Dynamic, Focusable, ForEach, Frame,
    Func, IntoProp, List, Memo, NodeRef, Portal, Prop, ReadSignal, RenderFn, ScopeContext, Show,
    WriteSignal, clone, component_accessibility, component_rect, component_size, create_effect,
    create_memo, create_signal, node_scope, on_cleanup, on_shortcut, owner_scope,
    set_component_state, try_with_document, with_document,
};
use crate::unstyled::{
    Choice, ChoiceKind, ChoiceOption, ChoiceOptionHandle, DragHandle, DragPoint, Draggable,
    DropHandle, DropTarget, Scroll,
};

pub use state::{
    DockDrop, DockLayout, DockSplitter, DockState, Entry, GroupId, LeafId, Side, SplitId,
    SurfaceId, TabId, TabPosition, Tree, layout_surface, layout_tree,
};
use state::{FLOATING_SIZE, MIN_WINDOW_SIZE, fraction_moved};

pub const SPLITTER_THICKNESS: f32 = 6.0;
const EDGE_ZONE: f32 = 0.22;
const GRIP: f32 = 7.0;
const MARKER_WIDTH: f32 = 3.0;
const SPLIT_STEP: f32 = 0.02;
const FLOAT_INSET: Vec2 = Vec2::new(64.0, 48.0);
const GRAB_OFFSET: Vec2 = Vec2::new(72.0, 14.0);
const GROUP_ZONE: f32 = 0.3;
const OUTER_EDGE: f32 = 18.0;
const WINDOW_KEEP_VISIBLE: f32 = 96.0;

pub struct DockTabHandle {
    pub entry: Entry,
    pub leaf: LeafId,
    pub index: usize,
    pub floating: bool,
    pub title: Memo<String>,
    pub tabs: Memo<Vec<TabId>>,
    pub has_next: Memo<bool>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub dragged: Memo<bool>,
    pub close: ClickCallback,
    pub float: ClickCallback,
    pub group: ClickCallback,
    pub split: ClickCallback,
    pub ungroup: ClickCallback,
}

pub struct DockPreviewHandle {
    pub entry: Entry,
    pub title: Memo<String>,
}

pub struct DockPanelHandle {
    pub leaf: LeafId,
    pub surface: SurfaceId,
    pub floating: bool,
    pub nested: bool,
    pub focused: Memo<bool>,
    pub bar: Option<NodeId>,
    pub body: NodeId,
}

pub struct DockSplitterHandle {
    pub direction: Direction,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct DockWindowGripHandle {
    pub surface: SurfaceId,
    pub focused: Memo<bool>,
}

pub struct DockWindowHandle {
    pub surface: SurfaceId,
    pub focused: Memo<bool>,
    pub title: Memo<String>,
    pub grip: NodeId,
    pub tabs: Option<NodeId>,
    pub close: ClickCallback,
    pub pane: NodeId,
}

#[derive(Clone, Default)]
struct TabBar {
    strip: NodeRef,
    tabs: NodeRef,
}

#[derive(Clone, Copy, PartialEq)]
struct Drag {
    entry: Entry,
    target: Option<DockDrop>,
    highlight: Option<Rect>,
}

struct State {
    state: ReadSignal<DockState>,
    set_state: WriteSignal<DockState>,
    on_change: Callback<DockState>,
    on_close: Callback<TabId>,
    drag: ReadSignal<Option<Drag>>,
    set_drag: WriteSignal<Option<Drag>>,
    title: Func<TabId, String>,
    thickness: f32,
    group_inset: f32,
    rect: ReadSignal<Rect>,
    panes: RefCell<HashMap<Tree, NodeRef>>,
    windows: RefCell<HashMap<SurfaceId, NodeRef>>,
    bars: RefCell<HashMap<LeafId, TabBar>>,
    panels: RefCell<HashMap<TabId, NodeId>>,
    owner: Option<ScopeContext>,
    tab: RenderFn<DockTabHandle>,
    content: RenderFn<TabId>,
    panel: RenderFn<DockPanelHandle>,
    splitter: RenderFn<DockSplitterHandle>,
    window_grip: RenderFn<DockWindowGripHandle>,
    window: RenderFn<DockWindowHandle>,
    highlight: RenderFn<()>,
    preview: RenderFn<DockPreviewHandle>,
}

type Handle = Rc<State>;

impl State {
    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next == current {
            return;
        }
        self.set_state.set(next.clone());
        self.on_change.call(next);
    }

    fn title(&self, tab: TabId) -> String {
        self.title.call(tab)
    }

    fn entry_title(&self, entry: Entry) -> String {
        match entry {
            Entry::Tab(tab) => self.title(tab),
            Entry::Group(group) => {
                let tabs = self.state.with(|state| state.group_tabs(group));
                group_title(tabs.into_iter().map(|tab| self.title(tab)).collect())
            }
        }
    }

    fn panel(&self, tab: TabId) -> NodeId {
        if let Some(panel) = self.panels.borrow().get(&tab) {
            return *panel;
        }
        let scope = with_document(|document| node_scope(document, self.owner.clone()));
        let panel = scope.context().run(|| self.content.call(tab));
        with_document(|document| document.register_node_scope(panel, scope));
        self.panels.borrow_mut().insert(tab, panel);
        panel
    }

    fn keep_panels(&self, tabs: &[TabId]) {
        let dropped: Vec<TabId> = self
            .panels
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
        let Some(panel) = self.panels.borrow_mut().remove(&tab) else {
            return;
        };
        try_with_document(|document| document.remove_node(panel));
    }

    fn pane_rect(&self, tree: Tree) -> Option<Rect> {
        let node = self.panes.borrow().get(&tree)?.try_get()?;
        with_document(|document| document.node_rect(node))
    }

    fn surface_rect(&self, surface: SurfaceId) -> Option<Rect> {
        let window = self
            .windows
            .borrow()
            .get(&surface)
            .and_then(NodeRef::try_get);
        match window {
            Some(window) => with_document(|document| document.node_rect(window)),
            None => self.pane_rect(Tree::Surface(surface)),
        }
    }

    fn tab_rects(&self, leaf: LeafId) -> Vec<Rect> {
        let Some(node) = self
            .bars
            .borrow()
            .get(&leaf)
            .and_then(|bar| bar.tabs.try_get())
        else {
            return Vec::new();
        };
        with_document(|document| {
            document
                .children(node)
                .into_iter()
                .filter_map(|child| document.node_rect(child))
                .collect()
        })
    }

    fn over_window_bar(&self, surface: SurfaceId, pos: Pos2) -> bool {
        let Some(pane) = self.pane_rect(Tree::Surface(surface)) else {
            return false;
        };
        if pos.y >= pane.top() {
            return false;
        }
        let leaves = self.state.get_untracked().leaves(surface);
        !leaves
            .into_iter()
            .flat_map(|leaf| self.tab_rects(leaf))
            .any(|tab| tab.contains(pos))
    }

    fn bar_rect(&self, leaf: LeafId) -> Option<Rect> {
        let node = self.bars.borrow().get(&leaf)?.strip.try_get()?;
        with_document(|document| document.node_rect(node))
    }

    fn close_tab(&self, tab: TabId) {
        self.edit(|state| {
            state.remove(tab);
        });
        self.on_close.call(tab);
    }

    fn close_entry(&self, entry: Entry) {
        let tabs = self.state.with_untracked(|state| state.entry_tabs(entry));
        for tab in tabs {
            self.close_tab(tab);
        }
    }

    fn float_entry(&self, entry: Entry) {
        let dock = self.rect.get_untracked();
        let origin = pos2(FLOAT_INSET.x, FLOAT_INSET.y);
        let pos = self.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), dock.size());
        self.edit(|state| state.drop_entry(entry, DockDrop::Window { pos }));
    }

    fn clamped_origin(&self, rect: Rect, bounds: Vec2) -> Pos2 {
        let x = rect.min.x.clamp(0.0, (bounds.x - rect.width()).max(0.0));
        let y = rect.min.y.clamp(0.0, (bounds.y - rect.height()).max(0.0));
        pos2(x, y)
    }

    fn reachable_origin(&self, rect: Rect, bounds: Vec2, bar: f32) -> Pos2 {
        let keep = rect.width().min(WINDOW_KEEP_VISIBLE);
        let x = rect
            .min
            .x
            .clamp(keep - rect.width(), (bounds.x - keep).max(0.0));
        let y = rect.min.y.clamp(0.0, (bounds.y - bar).max(0.0));
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

    fn begin_drag(&self, entry: Entry) {
        self.set_drag.set(Some(Drag {
            entry,
            target: None,
            highlight: None,
        }));
    }

    fn drag_over(&self, point: Option<DragPoint>) {
        let Some(mut drag) = self.drag.get_untracked() else {
            return;
        };
        let (target, highlight) = match point {
            Some(point) => self.resolve(point.pos, point.modifiers.alt, drag.entry),
            None => (None, None),
        };
        drag.target = target;
        drag.highlight = highlight;
        self.set_drag.set(Some(drag));
    }

    fn end_drag(&self) {
        self.set_drag.set(None);
    }

    fn drop_at(&self, entry: Entry, point: DragPoint) {
        let (target, _) = self.resolve(point.pos, point.modifiers.alt, entry);
        if let Some(target) = target {
            self.edit(|state| state.drop_entry(entry, target));
        }
    }

    fn resolve(&self, pos: Pos2, float: bool, dragged: Entry) -> (Option<DockDrop>, Option<Rect>) {
        let state = self.state.get_untracked();
        if !float {
            for surface in state.surfaces().into_iter().rev() {
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
        dragged: Entry,
    ) -> Option<(Option<DockDrop>, Option<Rect>)> {
        let area = self.pane_rect(tree)?;
        let layout = layout_tree(state, tree, area, self.thickness);
        let leaf = nearest_leaf(&layout, pos)?;
        let rect = layout.leaf_rect(leaf).unwrap_or(area);
        let bar = self
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
        dragged: Entry,
    ) -> (DockDrop, Rect) {
        let bar = self.bar_rect(leaf).unwrap_or(Rect::ZERO);
        let rects = self.tab_rects(leaf);
        let entries = state.entries(leaf);
        for (index, rect) in rects.iter().enumerate() {
            let across = (pos.x - rect.left()) / rect.width().max(1.0);
            let onto = entries.get(index).is_some_and(|onto| *onto != dragged);
            if onto && (GROUP_ZONE..=1.0 - GROUP_ZONE).contains(&across) {
                return (DockDrop::Group { leaf, index }, *rect);
            }
            if pos.x < rect.center().x {
                return (DockDrop::Tab { leaf, index }, marker_rect(rect.left(), bar));
            }
        }
        let end = rects.last().map_or(bar.left(), Rect::right);
        (
            DockDrop::Tab {
                leaf,
                index: rects.len(),
            },
            marker_rect(end, bar),
        )
    }

    fn splitter_of(&self, tree: Tree, split: SplitId) -> Option<DockSplitter> {
        let area = self.pane_rect(tree)?;
        let state = self.state.get_untracked();
        layout_tree(&state, tree, area, self.thickness).splitter(split)
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

fn marker_rect(x: f32, bar: Rect) -> Rect {
    let x = x.clamp(bar.left(), bar.right());
    Rect::from_min_max(
        pos2(x - MARKER_WIDTH / 2.0, bar.top()),
        pos2(x + MARKER_WIDTH / 2.0, bar.bottom()),
    )
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

const GRIPS: [Grip; 8] = [
    Grip::Left,
    Grip::Right,
    Grip::Top,
    Grip::Bottom,
    Grip::TopLeft,
    Grip::TopRight,
    Grip::BottomLeft,
    Grip::BottomRight,
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

    fn rect(self, size: Vec2) -> Rect {
        let far = pos2(size.x - GRIP, size.y - GRIP);
        let corner = vec2(GRIP, GRIP);
        match self {
            Grip::Left => Rect::from_min_size(pos2(0.0, GRIP), vec2(GRIP, size.y - GRIP * 2.0)),
            Grip::Right => Rect::from_min_size(pos2(far.x, GRIP), vec2(GRIP, size.y - GRIP * 2.0)),
            Grip::Top => Rect::from_min_size(pos2(GRIP, 0.0), vec2(size.x - GRIP * 2.0, GRIP)),
            Grip::Bottom => Rect::from_min_size(pos2(GRIP, far.y), vec2(size.x - GRIP * 2.0, GRIP)),
            Grip::TopLeft => Rect::from_min_size(Pos2::ZERO, corner),
            Grip::TopRight => Rect::from_min_size(pos2(far.x, 0.0), corner),
            Grip::BottomLeft => Rect::from_min_size(pos2(0.0, far.y), corner),
            Grip::BottomRight => Rect::from_min_size(far, corner),
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

#[component]
pub fn Dock(
    state: Prop<DockState>,
    on_change: Callback<DockState>,
    on_close: Callback<TabId>,
    title: Func<TabId, String>,
    #[prop(default = SPLITTER_THICKNESS)] splitter_thickness: f32,
    #[prop(default = 0.0)] group_inset: f32,
    tab: RenderFn<DockTabHandle>,
    #[prop(children)] content: RenderFn<TabId>,
    panel: Option<RenderFn<DockPanelHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    window_grip: Option<RenderFn<DockWindowGripHandle>>,
    window: Option<RenderFn<DockWindowHandle>>,
    highlight: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
) -> NodeId {
    let (current, set_current) = create_signal(state.peek());
    create_effect(clone!(set_current -> move || set_current.set(state.get())));
    let (drag, set_drag) = create_signal(None);
    let dock: Handle = Rc::new(State {
        state: current.clone(),
        windows: RefCell::default(),
        panels: RefCell::default(),
        owner: owner_scope(),
        set_state: set_current,
        on_change,
        on_close,
        drag,
        set_drag,
        title,
        thickness: splitter_thickness,
        group_inset,
        rect: component_rect(),
        panes: RefCell::default(),
        bars: RefCell::default(),
        tab,
        content,
        panel: panel.unwrap_or_else(|| {
            RenderFn::new(|handle| {
                view! {
                    <StackedPanel handle />
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
        window_grip: window_grip.unwrap_or_else(|| {
            RenderFn::new(|_| {
                view! {
                    <Frame />
                }
            })
        }),
        window: window.unwrap_or_else(|| {
            RenderFn::new(|handle| {
                view! {
                    <StackedWindow handle />
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
    });
    set_component_state(dock.clone());
    on_shortcut(clone!(dock -> move |press: KeyPress| {
        if !press.pressed || press.key != Key::Tab || !press.modifiers.ctrl || press.modifiers.alt
        {
            return false;
        }
        dock.cycle(press.modifiers.shift)
    }));
    create_effect(clone!(dock current -> move || {
        let tabs = current.with(DockState::all_tabs);
        dock.keep_panels(&tabs);
    }));
    on_cleanup(clone!(dock -> move || dock.keep_panels(&[])));
    let main = create_memo(clone!(current -> move || current.with(DockState::main)));
    let windows = create_memo(clone!(current -> move || current.with(DockState::windows)));
    let panes = dock.clone();
    let floating = dock.clone();
    let over = dock.clone();
    let dropped = dock.clone();
    view! {
        <DropTarget
            on_over={move |point: Option<(Entry, DragPoint)>| over.drag_over(point.map(|(_, point)| point))}
            on_drop={move |(entry, point): (Entry, DragPoint)| dropped.drop_at(entry, point)}
        >
            {move |_: DropHandle| view! {
                <List spacing=0.0>
                    <Dynamic value={main}>
                        {move |surface: SurfaceId| {
                            let dock = panes.clone();
                            view! {
                                <DockPane
                                    dock
                                    tree={Tree::Surface(surface)}
                                    @sizing=ItemSize::Percent(100.0)
                                />
                            }
                        }}
                    </Dynamic>
                    <ForEach keys={windows}>
                        {move |surface: SurfaceId| {
                            let dock = floating.clone();
                            view! {
                                <DockWindowView dock surface />
                            }
                        }}
                    </ForEach>
                    <DockDragLayer dock />
                </List>
            }}
        </DropTarget>
    }
}

#[component]
fn StackedPanel(handle: DockPanelHandle) -> NodeId {
    let DockPanelHandle { bar, body, .. } = handle;
    view! {
        <List spacing=0.0>
            <Show condition={bar.is_some()}>
                {bar.expect("the panel keeps its own tab bar")}
            </Show>
            {body} @sizing=ItemSize::Percent(100.0)
        </List>
    }
}

#[component]
fn StackedWindow(handle: DockWindowHandle) -> NodeId {
    let DockWindowHandle {
        grip, tabs, pane, ..
    } = handle;
    view! {
        <List spacing=0.0>
            <List direction=Direction::Horizontal spacing=0.0>
                {grip}
                <Show condition={tabs.is_some()}>
                    {tabs.expect("the window holds one pane")}
                </Show>
            </List>
            {pane} @sizing=ItemSize::Percent(100.0)
        </List>
    }
}

#[component]
fn DockPane(
    dock: Handle,
    tree: Tree,
    #[prop(default = None)] hoisted: Prop<Option<LeafId>>,
) -> NodeId {
    let canvas = NodeRef::new();
    dock.panes.borrow_mut().insert(tree, canvas.clone());
    on_cleanup(clone!(dock canvas -> move || {
        let mut panes = dock.panes.borrow_mut();
        if panes.get(&tree).is_some_and(|current| current.try_get() == canvas.try_get()) {
            panes.remove(&tree);
        }
    }));
    let size = component_size();
    let placement = component_rect();
    let thickness = dock.thickness;
    let state = dock.state.clone();
    let layout = create_memo(clone!(state -> move || {
        let area = Rect::from_min_size(Pos2::ZERO, size.get());
        state.with(|state| layout_tree(state, tree, area, thickness))
    }));
    let leaves = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.leaves.iter().map(|(leaf, _)| *leaf).collect::<Vec<_>>())
    }));
    let splits = create_memo(clone!(layout -> move || {
        layout.with(|layout| layout.splitters.iter().map(|splitter| splitter.id).collect::<Vec<_>>())
    }));
    let hoisted = hoisted.peek();
    let panels = dock.clone();
    let panel_layout = layout.clone();
    let splitters = dock.clone();
    let splitter_layout = layout.clone();
    let marked = dock.clone();
    let marked_surface = match tree {
        Tree::Surface(surface) => state
            .with_untracked(|state| state.window_rect(surface).is_none())
            .then_some(surface),
        Tree::Group(_) => None,
    };
    let pane_origin = create_memo(clone!(placement -> move || placement.get().min));
    view! {
        <Canvas @node_ref=&canvas>
            <ForEach keys={leaves}>
                {move |leaf: LeafId| {
                    let dock = panels.clone();
                    let layout = panel_layout.clone();
                    let rect = create_memo(clone!(layout -> move || {
                        layout.with(|layout| layout.leaf_rect(leaf)).unwrap_or(Rect::ZERO)
                    }));
                    let x = create_memo(clone!(rect -> move || rect.get().min.x));
                    let y = create_memo(clone!(rect -> move || rect.get().min.y));
                    let width = create_memo(clone!(rect -> move || rect.get().width()));
                    let height = create_memo(clone!(rect -> move || rect.get().height()));
                    let hoisted = hoisted == Some(leaf);
                    view! {
                        <CanvasItem x={x} y={y} width={width} height={height}>
                            <DockPanelView dock tree leaf hoisted />
                        </CanvasItem>
                    }
                }}
            </ForEach>
            <ForEach keys={splits}>
                {move |split: SplitId| {
                    let dock = splitters.clone();
                    let layout = splitter_layout.clone();
                    let rect = create_memo(clone!(layout -> move || {
                        layout.with(|layout| layout.splitter(split)).map_or(Rect::ZERO, |splitter| splitter.handle)
                    }));
                    let x = create_memo(clone!(rect -> move || rect.get().min.x));
                    let y = create_memo(clone!(rect -> move || rect.get().min.y));
                    let width = create_memo(clone!(rect -> move || rect.get().width()));
                    let height = create_memo(clone!(rect -> move || rect.get().height()));
                    view! {
                        <CanvasItem x={x} y={y} width={width} height={height}>
                            <DockSplitterView dock tree split />
                        </CanvasItem>
                    }
                }}
            </ForEach>
            <Show condition={marked_surface.is_some()}>
                <DockDropMarker
                    dock={marked}
                    surface={marked_surface.unwrap_or_else(|| unreachable!())}
                    origin={pane_origin}
                />
            </Show>
        </Canvas>
    }
}

#[component]
fn DockDropMarker(dock: Handle, surface: SurfaceId, origin: Memo<Pos2>) -> CanvasItem {
    let marker = create_memo(clone!(dock origin -> move || {
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
    }));
    let shown = create_memo(clone!(marker -> move || marker.get().is_some()));
    let rect = create_memo(clone!(marker -> move || marker.get().unwrap_or(Rect::ZERO)));
    let x = create_memo(clone!(rect -> move || rect.get().min.x));
    let y = create_memo(clone!(rect -> move || rect.get().min.y));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame visible={shown}>{dock.highlight.call(())}</Frame>
        </CanvasItem>
    }
}

#[component]
fn DockPanelView(dock: Handle, tree: Tree, leaf: LeafId, hoisted: bool) -> NodeId {
    let state = dock.state.clone();
    let focused = create_memo(clone!(state -> move || {
        state.with(|state| state.focused_leaf() == Some(leaf))
    }));
    let surface = state
        .with_untracked(|state| state.surface_of(leaf))
        .unwrap_or_else(|| state.with_untracked(DockState::main));
    let nested = matches!(tree, Tree::Group(_));
    let floating = !nested && state.with_untracked(|state| state.window_rect(surface).is_some());
    let bar = match hoisted {
        true => None,
        false => Some(view! {
            <DockTabBar dock={dock.clone()} leaf />
        }),
    };
    let body = view! {
        <DockPanelBody dock={dock.clone()} leaf />
    };
    let chrome = dock.panel.call(DockPanelHandle {
        leaf,
        surface,
        floating,
        nested,
        focused,
        bar,
        body,
    });
    let pressed = dock.clone();
    view! {
        <ClickCatcher
            on_press={move |press: PointerPress| {
                let hosted = pressed.state.with_untracked(|state| state.active_entry(leaf));
                let inside = hosted
                    .and_then(Entry::group)
                    .and_then(|group| pressed.pane_rect(Tree::Group(group)))
                    .is_some_and(|inner| inner.contains(press.pos));
                if !inside {
                    pressed.edit(|state| state.focus(leaf));
                }
            }}
            children={chrome}
        />
    }
}

#[component]
fn DockPanelBody(dock: Handle, leaf: LeafId) -> NodeId {
    let state = dock.state.clone();
    let group = create_memo(clone!(state -> move || {
        state.with(|state| state.active_entry(leaf).and_then(Entry::group))
    }));
    view! {
        <List spacing=0.0>
            <Dynamic value={group}>
                {move |group: Option<GroupId>| {
                    let dock = dock.clone();
                    let inset = dock.group_inset;
                    match group {
                        Some(group) => view! {
                            <Frame padding_horizontal={inset} @sizing=ItemSize::Percent(100.0)>
                                <List spacing=0.0>
                                    <DockPane
                                        dock
                                        tree={Tree::Group(group)}
                                        @sizing=ItemSize::Percent(100.0)
                                    />
                                    <Frame height={inset} />
                                </List>
                            </Frame>
                        },
                        None => view! {
                            <DockTabBody dock leaf @sizing=ItemSize::Percent(100.0) />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn DockTabBody(dock: Handle, leaf: LeafId) -> NodeId {
    let state = dock.state.clone();
    let shown = create_memo(clone!(state -> move || state.with(|state| state.active_tab(leaf))));
    let (panel, set_panel) = create_signal(None);
    create_effect(clone!(dock -> move || {
        set_panel.set(shown.get().map(|tab| dock.panel(tab)));
    }));
    view! {
        <Portal node={panel} />
    }
}

#[component]
fn DockTabBar(dock: Handle, leaf: LeafId) -> NodeId {
    let bar = TabBar::default();
    let (strip, tab_list) = (bar.strip.clone(), bar.tabs.clone());
    dock.bars.borrow_mut().insert(leaf, bar);
    on_cleanup(clone!(dock -> move || {
        dock.bars.borrow_mut().remove(&leaf);
    }));
    let state = dock.state.clone();
    let entries = create_memo(clone!(state -> move || state.with(|state| state.entries(leaf))));
    let selected = create_memo(clone!(state -> move || {
        Some(state.with(|state| state.active_index(leaf)))
    }));
    let labels = dock.clone();
    let options = view! {
        <ForEach keys={entries}>
            {move |entry: Entry| {
                let dock = labels.clone();
                let label = create_memo(move || dock.entry_title(entry));
                view! {
                    <ChoiceOption label={label} />
                }
            }}
        </ForEach>
    };
    let changed = dock.clone();
    let faces = dock.clone();
    view! {
        <Scroll direction=Direction::Horizontal @node_ref=&strip>
            <Choice
                @node_ref=&tab_list
                options={options}
                selected={selected}
                kind=ChoiceKind::Tabs
                on_change={move |index: Option<usize>| {
                    let Some(index) = index else {
                        return;
                    };
                    changed.edit(|state| {
                        state.set_active_index(leaf, index);
                        state.focus(leaf);
                    });
                }}
            >
                {move |handle: ChoiceOptionHandle| {
                    let dock = faces.clone();
                    let entry = dock.state.get_untracked().entries(leaf).get(handle.index).copied();
                    match entry {
                        Some(entry) => view! {
                            <DockTabView dock leaf entry handle />
                        },
                        None => view! {
                            <Frame />
                        },
                    }
                }}
            </Choice>
        </Scroll>
    }
}

#[component]
fn DockTabView(dock: Handle, leaf: LeafId, entry: Entry, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        index,
        selected,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let state = dock.state.clone();
    let title = create_memo(clone!(dock -> move || dock.entry_title(entry)));
    let tabs = create_memo(clone!(state -> move || state.with(|state| state.entry_tabs(entry))));
    let has_next = create_memo(clone!(state -> move || {
        state.with(|state| {
            let entries = state.entries(leaf);
            entries
                .iter()
                .position(|candidate| *candidate == entry)
                .is_some_and(|index| index + 1 < entries.len())
        })
    }));
    let dragged = create_memo(clone!(dock -> move || {
        dock.drag
            .with(|drag| drag.as_ref().is_some_and(|drag| drag.entry == entry))
    }));
    let close = ClickCallback::new(clone!(dock -> move || dock.close_entry(entry)));
    let float = ClickCallback::new(clone!(dock -> move || dock.float_entry(entry)));
    let group = ClickCallback::new(clone!(dock -> move || {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.group_with_next(leaf, index);
            }
        });
    }));
    let split = ClickCallback::new(clone!(dock -> move || {
        dock.edit(|state| {
            if let Some((leaf, index)) = state.locate(entry) {
                state.split_with_next(leaf, index);
            }
        });
    }));
    let ungroup = ClickCallback::new(clone!(dock -> move || {
        if let Entry::Group(group) = entry {
            dock.edit(|state| state.ungroup(group));
        }
    }));
    let floating = dock.state.with_untracked(|state| {
        !state.is_nested(leaf)
            && state
                .surface_of(leaf)
                .and_then(|surface| state.window_rect(surface))
                .is_some()
    });
    let face = dock.tab.call(DockTabHandle {
        entry,
        leaf,
        index,
        floating,
        title: title.clone(),
        tabs,
        has_next,
        selected,
        hovered,
        active,
        focused,
        dragged,
        close,
        float,
        group,
        split,
        ungroup,
    });
    let carried = dock.clone();
    let preview = dock.preview.clone();
    view! {
        <Draggable
            payload={entry}
            preview={move |entry: Entry| preview.call(DockPreviewHandle {
                entry,
                title: title.clone(),
            })}
            on_drag_change={move |dragging: bool| match dragging {
                true => carried.begin_drag(entry),
                false => carried.end_drag(),
            }}
        >
            {move |_: DragHandle| face}
        </Draggable>
    }
}

#[component]
fn DockSplitterView(dock: Handle, tree: Tree, split: SplitId) -> NodeId {
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let state = dock.state.clone();
    let direction = state
        .with_untracked(|state| state.split_direction(split))
        .unwrap_or(Direction::Horizontal);
    let fraction =
        create_memo(clone!(state -> move || state.with(|state| state.split_fraction(split))));
    component_accessibility(create_memo(clone!(fraction -> move || {
        let mut node = Node::new(Role::Splitter);
        node.set_numeric_value(f64::from(fraction.get() * 100.0));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(100.0);
        node
    })));
    let face = dock.splitter.call(DockSplitterHandle {
        direction,
        hovered: hovered.clone(),
        active: active.clone(),
        focused: focused.clone(),
    });
    let held: Rc<Cell<Option<(f32, Pos2)>>> = Rc::new(Cell::new(None));
    let grabbed = held.clone();
    let start = fraction.clone();
    let dragged = dock.clone();
    let stepped = dock.clone();
    view! {
        <Focusable
            on_focus_change={move |has_focus: bool| set_focused.set(has_focus)}
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
                let next = fraction.get_untracked() + step;
                stepped.edit(|state| state.set_split_fraction(split, next));
                true
            }}
        >
            <ClickCatcher
                cursor={match direction {
                    Direction::Horizontal => CursorIcon::ResizeHorizontal,
                    Direction::Vertical => CursorIcon::ResizeVertical,
                }}
                on_hover_change={move |over: bool| set_hovered.set(over)}
                on_active_change={move |held: bool| set_active.set(held)}
                on_press={move |press: PointerPress| {
                    grabbed.set(Some((start.get_untracked(), press.pos)));
                }}
                on_drag={move |press: PointerPress| {
                    let Some((start, from)) = held.get() else {
                        return;
                    };
                    let moved = direction.main(press.pos - from);
                    if moved == 0.0 {
                        return;
                    }
                    let Some(splitter) = dragged.splitter_of(tree, split) else {
                        return;
                    };
                    let fraction = fraction_moved(
                        splitter.area,
                        splitter.direction,
                        dragged.thickness,
                        start,
                        moved,
                    );
                    dragged.edit(|state| state.set_split_fraction(split, fraction));
                }}
                children={face}
            />
        </Focusable>
    }
}

#[component]
fn DockWindowView(dock: Handle, surface: SurfaceId) -> NodeId {
    let frame = NodeRef::new();
    dock.windows.borrow_mut().insert(surface, frame.clone());
    on_cleanup(clone!(dock -> move || {
        dock.windows.borrow_mut().remove(&surface);
    }));
    let state = dock.state.clone();
    let rect = create_memo(clone!(state -> move || {
        state.with(|state| state.window_rect(surface)).unwrap_or(Rect::ZERO)
    }));
    let area = dock.rect.clone();
    let origin =
        create_memo(clone!(rect area -> move || area.get().min + rect.get().min.to_vec2()));
    let anchor = origin.clone().into_prop().map(OverlayAnchor::Point);
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    let size = create_memo(clone!(rect -> move || rect.get().size()));
    let focused = create_memo(clone!(state -> move || {
        state.with(|state| {
            state
                .focused_leaf()
                .and_then(|leaf| state.surface_of(leaf))
                .is_some_and(|focused| focused == surface)
        })
    }));
    let title = create_memo(clone!(dock state -> move || {
        state
            .with(|state| {
                state
                    .leaves(surface)
                    .first()
                    .and_then(|leaf| state.active_entry(*leaf))
            })
            .map(|entry| dock.entry_title(entry))
            .unwrap_or_default()
    }));
    let close = ClickCallback::new(clone!(dock -> move || {
        for tab in dock.state.get_untracked().surface_tabs(surface) {
            dock.close_tab(tab);
        }
    }));
    let grip_face = dock.window_grip.call(DockWindowGripHandle {
        surface,
        focused: focused.clone(),
    });
    let grip = view! {
        <ClickCatcher cursor=CursorIcon::Grab children={grip_face} />
    };
    let hoisted = state.with_untracked(|state| state.leaves(surface).first().copied());
    let tabs = hoisted.map(|leaf| {
        view! {
            <DockTabBar dock={dock.clone()} leaf />
        }
    });
    let grips = dock.clone();
    let marker = dock.clone();
    let grip_rect = rect.clone();
    let overlay = NodeRef::new();
    create_effect(clone!(focused overlay -> move || {
        if !focused.get() {
            return;
        }
        let Some(overlay) = overlay.try_get() else {
            return;
        };
        with_document(|document| document.raise_overlay(overlay));
    }));
    let pane = view! {
        <DockPane dock={dock.clone()} tree={Tree::Surface(surface)} hoisted />
    };
    let chrome = dock.window.call(DockWindowHandle {
        surface,
        focused,
        title,
        grip,
        tabs,
        close,
        pane,
    });
    let grabbed: Rc<Cell<Option<(Rect, Pos2, f32)>>> = Rc::new(Cell::new(None));
    let bar_rect = rect.clone();
    let pressed = dock.clone();
    let start = grabbed.clone();
    let moved = dock.clone();
    view! {
        <Overlay
            @node_ref=&overlay
            anchor={anchor}
            placement=Placement::At
            mode=OverlayMode::Floating
            traps_focus=false
            open=true
        >
            <Frame @node_ref=&frame width={width.clone()} height={height.clone()}>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                        <ClickCatcher
                            on_press={move |press: PointerPress| {
                                let bar = pressed.over_window_bar(surface, press.pos);
                                start.set(bar.then(|| {
                                    let window = bar_rect.get_untracked();
                                    let top = pressed.rect.get_untracked().min.y + window.min.y;
                                    let height = pressed
                                        .pane_rect(Tree::Surface(surface))
                                        .map_or(0.0, |pane| pane.top() - top);
                                    (window, press.pos, height)
                                }));
                                let titled = pressed
                                    .pane_rect(Tree::Surface(surface))
                                    .is_some_and(|pane| press.pos.y < pane.top());
                                pressed.edit(|state| {
                                    let inside = state
                                        .focused_leaf()
                                        .and_then(|leaf| state.surface_of(leaf))
                                        == Some(surface);
                                    if inside && !titled {
                                        state.raise(surface);
                                        return;
                                    }
                                    if let Some(leaf) = state.leaves(surface).first().copied() {
                                        state.focus(leaf);
                                    }
                                });
                            }}
                            on_drag={move |press: PointerPress| {
                                let Some((start, from, bar)) = grabbed.get() else {
                                    return;
                                };
                                let placed =
                                    Rect::from_min_size(start.min + (press.pos - from), start.size());
                                let bounds = moved.rect.get_untracked().size();
                                let origin = moved.reachable_origin(placed, bounds, bar);
                                moved.edit(|state| {
                                    state.set_window_rect(
                                        surface,
                                        Rect::from_min_size(origin, start.size()),
                                    );
                                });
                            }}
                            children={chrome}
                        />
                    </CanvasItem>
                    <DockDropMarker dock={marker} surface origin={origin} />
                    <ForEach keys={GRIPS.to_vec()}>
                        {move |grip: Grip| {
                            let dock = grips.clone();
                            let size = size.clone();
                            let rect = grip_rect.clone();
                            let handle = create_memo(clone!(size -> move || grip.rect(size.get())));
                            let x = create_memo(clone!(handle -> move || handle.get().min.x));
                            let y = create_memo(clone!(handle -> move || handle.get().min.y));
                            let width = create_memo(clone!(handle -> move || handle.get().width()));
                            let height = create_memo(clone!(handle -> move || handle.get().height()));
                            let held: Rc<Cell<(Rect, Pos2)>> = Rc::new(Cell::new((Rect::ZERO, Pos2::ZERO)));
                            let pressed = held.clone();
                            let rect = rect.clone();
                            view! {
                                <CanvasItem x={x} y={y} width={width} height={height}>
                                    <ClickCatcher
                                        cursor={grip.cursor()}
                                        on_press={move |press: PointerPress| {
                                            pressed.set((rect.get_untracked(), press.pos));
                                        }}
                                        on_drag={move |press: PointerPress| {
                                            let (start, from) = held.get();
                                            let resized = grip.resized(start, press.pos - from);
                                            dock.edit(|state| state.set_window_rect(surface, resized));
                                        }}
                                    ></ClickCatcher>
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                </Canvas>
            </Frame>
        </Overlay>
    }
}

#[component]
fn DockDragLayer(dock: Handle) -> NodeId {
    let drag = dock.drag.clone();
    let marked = create_memo(clone!(drag -> move || {
        drag.with(|drag| {
            let drag = drag.as_ref()?;
            matches!(drag.target?, DockDrop::Window { .. })
                .then_some(drag.highlight)
                .flatten()
        })
    }));
    let marking = create_memo(clone!(marked -> move || marked.get().is_some()));
    let highlight = create_memo(clone!(marked -> move || marked.get().unwrap_or(Rect::ZERO)));
    let marker_anchor = create_memo(clone!(highlight -> move || highlight.get().min))
        .into_prop()
        .map(OverlayAnchor::Point);
    let marker_width = create_memo(clone!(highlight -> move || highlight.get().width()));
    let marker_height = create_memo(clone!(highlight -> move || highlight.get().height()));
    let face = dock.highlight.call(());
    view! {
        <List spacing=0.0>
            <Overlay
                anchor={marker_anchor}
                placement=Placement::BelowStart
                mode=OverlayMode::Passive
                traps_focus=false
                open={marking}
            >
                <Frame width={marker_width} height={marker_height}>{face}</Frame>
            </Overlay>
        </List>
    }
}

pub fn dock_state(document: &Document, dock: NodeId) -> DockState {
    document.component_state::<Handle>(dock).state.get()
}
