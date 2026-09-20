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
    Func, IntoProp, List, Memo, NodeRef, Prop, ReadSignal, RenderFn, Show, WriteSignal, clone,
    component_accessibility, component_rect, component_size, create_effect, create_memo,
    create_signal, on_cleanup, set_component_state, with_document,
};
use crate::unstyled::{Choice, ChoiceKind, ChoiceOption, ChoiceOptionHandle, Scroll};

pub use state::{
    DockLayout, DockSplitter, DockState, DropTarget, LeafId, Side, SplitId, SurfaceId, TabId,
    TabPosition, layout_surface,
};
use state::{FLOATING_SIZE, MIN_WINDOW_SIZE, fraction_at};

pub const SPLITTER_THICKNESS: f32 = 6.0;
const DRAG_THRESHOLD: f32 = 4.0;
const EDGE_ZONE: f32 = 0.22;
const GRIP: f32 = 7.0;
const MARKER_WIDTH: f32 = 3.0;
const SPLIT_STEP: f32 = 0.02;
const PREVIEW_OFFSET: Vec2 = Vec2::new(12.0, 14.0);
const FLOAT_INSET: Vec2 = Vec2::new(64.0, 48.0);
const GRAB_OFFSET: Vec2 = Vec2::new(72.0, 14.0);

pub struct DockTabHandle {
    pub tab: TabId,
    pub leaf: LeafId,
    pub index: usize,
    pub title: Memo<String>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub dragged: Memo<bool>,
    pub close: ClickCallback,
}

pub struct DockPanelHandle {
    pub leaf: LeafId,
    pub surface: SurfaceId,
    pub floating: bool,
    pub focused: Memo<bool>,
    pub bar: Option<NodeId>,
    pub body: NodeId,
    pub float: ClickCallback,
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

#[derive(Clone, Copy, PartialEq)]
struct Drag {
    tab: TabId,
    from: Pos2,
    pointer: Pos2,
    moved: bool,
    target: Option<DropTarget>,
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
    rect: ReadSignal<Rect>,
    panes: RefCell<HashMap<SurfaceId, NodeRef>>,
    bars: RefCell<HashMap<LeafId, NodeRef>>,
    tab: RenderFn<DockTabHandle>,
    content: RenderFn<TabId>,
    panel: RenderFn<DockPanelHandle>,
    splitter: RenderFn<DockSplitterHandle>,
    window_grip: RenderFn<DockWindowGripHandle>,
    window: RenderFn<DockWindowHandle>,
    highlight: RenderFn<()>,
    preview: RenderFn<TabId>,
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

    fn pane_rect(&self, surface: SurfaceId) -> Option<Rect> {
        let node = self.panes.borrow().get(&surface)?.try_get()?;
        with_document(|document| document.node_rect(node))
    }

    fn bar_rect(&self, leaf: LeafId) -> Option<Rect> {
        let node = self.bars.borrow().get(&leaf)?.try_get()?;
        with_document(|document| document.node_rect(node))
    }

    fn close_tab(&self, tab: TabId) {
        self.edit(|state| {
            state.remove(tab);
        });
        self.on_close.call(tab);
    }

    fn float_tab(&self, tab: TabId) {
        let dock = self.rect.get_untracked();
        let origin = pos2(FLOAT_INSET.x, FLOAT_INSET.y);
        let pos = self.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), dock.size());
        self.edit(|state| state.drop_tab(tab, DropTarget::Window { pos }));
    }

    fn clamped_origin(&self, rect: Rect, bounds: Vec2) -> Pos2 {
        let x = rect.min.x.clamp(0.0, (bounds.x - rect.width()).max(0.0));
        let y = rect.min.y.clamp(0.0, (bounds.y - rect.height()).max(0.0));
        pos2(x, y)
    }

    fn begin_drag(&self, tab: TabId, pos: Pos2) {
        self.set_drag.set(Some(Drag {
            tab,
            from: pos,
            pointer: pos,
            moved: false,
            target: None,
            highlight: None,
        }));
    }

    fn drag_to(&self, pos: Pos2, float: bool) {
        let Some(mut drag) = self.drag.get_untracked() else {
            return;
        };
        drag.pointer = pos;
        drag.moved = drag.moved || drag.from.distance(pos) > DRAG_THRESHOLD;
        if drag.moved {
            let (target, highlight) = self.resolve(pos, float);
            drag.target = target;
            drag.highlight = highlight;
        }
        self.set_drag.set(Some(drag));
    }

    fn end_drag(&self) {
        let Some(drag) = self.drag.get_untracked() else {
            return;
        };
        self.set_drag.set(None);
        if !drag.moved {
            return;
        }
        let Some(target) = drag.target else {
            return;
        };
        self.edit(|state| state.drop_tab(drag.tab, target));
    }

    fn resolve(&self, pos: Pos2, float: bool) -> (Option<DropTarget>, Option<Rect>) {
        let state = self.state.get_untracked();
        if !float {
            for surface in state.surfaces().into_iter().rev() {
                let Some(area) = self.pane_rect(surface) else {
                    continue;
                };
                if !area.contains(pos) {
                    continue;
                }
                let layout = layout_surface(&state, surface, area, self.thickness);
                let Some(leaf) = nearest_leaf(&layout, pos) else {
                    continue;
                };
                let rect = layout.leaf_rect(leaf).unwrap_or(area);
                let bar = self
                    .bar_rect(leaf)
                    .is_some_and(|bar| pos.y >= bar.top() && pos.y <= bar.bottom());
                if bar {
                    let (index, marker) = self.insert_index(leaf, pos);
                    return (Some(DropTarget::Tab { leaf, index }), Some(marker));
                }
                let split = state.window_rect(surface).is_none();
                return match zone(rect, pos).filter(|_| split) {
                    Some(side) => (
                        Some(DropTarget::Split { leaf, side }),
                        Some(side_rect(rect, side)),
                    ),
                    None => (Some(DropTarget::Pane { leaf }), Some(rect)),
                };
            }
        }
        let dock = self.rect.get_untracked();
        let origin = pos - dock.min.to_vec2() - GRAB_OFFSET;
        let origin = self.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), dock.size());
        (
            Some(DropTarget::Window { pos: origin }),
            Some(Rect::from_min_size(
                dock.min + origin.to_vec2(),
                FLOATING_SIZE,
            )),
        )
    }

    fn insert_index(&self, leaf: LeafId, pos: Pos2) -> (usize, Rect) {
        let bar = self.bar_rect(leaf).unwrap_or(Rect::ZERO);
        let Some(node) = self.bars.borrow().get(&leaf).and_then(NodeRef::try_get) else {
            return (0, marker_rect(bar.left(), bar));
        };
        let rects: Vec<Rect> = with_document(|document| {
            document
                .children(node)
                .into_iter()
                .filter_map(|child| document.node_rect(child))
                .collect()
        });
        for (index, rect) in rects.iter().enumerate() {
            if pos.x < rect.center().x {
                return (index, marker_rect(rect.left(), bar));
            }
        }
        let edge = rects.last().map_or(bar.left(), Rect::right);
        (rects.len(), marker_rect(edge, bar))
    }

    fn splitter_of(&self, surface: SurfaceId, split: SplitId) -> Option<DockSplitter> {
        let area = self.pane_rect(surface)?;
        let state = self.state.get_untracked();
        layout_surface(&state, surface, area, self.thickness).splitter(split)
    }
}

fn marker_rect(x: f32, bar: Rect) -> Rect {
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
    tab: RenderFn<DockTabHandle>,
    #[prop(children)] content: RenderFn<TabId>,
    panel: Option<RenderFn<DockPanelHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    window_grip: Option<RenderFn<DockWindowGripHandle>>,
    window: Option<RenderFn<DockWindowHandle>>,
    highlight: Option<RenderFn<()>>,
    preview: Option<RenderFn<TabId>>,
) -> NodeId {
    let (current, set_current) = create_signal(state.peek());
    create_effect(clone!(set_current -> move || set_current.set(state.get())));
    let (drag, set_drag) = create_signal(None);
    let dock: Handle = Rc::new(State {
        state: current.clone(),
        set_state: set_current,
        on_change,
        on_close,
        drag,
        set_drag,
        title,
        thickness: splitter_thickness,
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
    let main = create_memo(clone!(current -> move || current.with(DockState::main)));
    let windows = create_memo(clone!(current -> move || current.with(DockState::windows)));
    let panes = dock.clone();
    let floating = dock.clone();
    view! {
        <List spacing=0.0>
            <Dynamic value={main}>
                {move |surface: SurfaceId| {
                    let dock = panes.clone();
                    view! {
                        <DockPane dock surface @sizing=ItemSize::Percent(100.0) />
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
    surface: SurfaceId,
    #[prop(default = None)] hoisted: Prop<Option<LeafId>>,
) -> NodeId {
    let canvas = NodeRef::new();
    dock.panes.borrow_mut().insert(surface, canvas.clone());
    on_cleanup(clone!(dock -> move || {
        dock.panes.borrow_mut().remove(&surface);
    }));
    let size = component_size();
    let placement = component_rect();
    let thickness = dock.thickness;
    let state = dock.state.clone();
    let layout = create_memo(move || {
        let area = Rect::from_min_size(Pos2::ZERO, size.get());
        state.with(|state| layout_surface(state, surface, area, thickness))
    });
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
    let marker = create_memo(clone!(dock placement -> move || {
        let state = dock.state.get_untracked();
        let drag = dock.drag.get()?;
        let leaf = match drag.target? {
            DropTarget::Tab { leaf, .. }
            | DropTarget::Pane { leaf }
            | DropTarget::Split { leaf, .. } => leaf,
            DropTarget::Window { .. } => return None,
        };
        if state.surface_of(leaf) != Some(surface) {
            return None;
        }
        let origin = placement.get().min.to_vec2();
        let highlight = drag.highlight?;
        Some(Rect::from_min_size(highlight.min - origin, highlight.size()))
    }));
    let marker_shown = create_memo(clone!(marker -> move || marker.get().is_some()));
    let marker_rect = create_memo(clone!(marker -> move || marker.get().unwrap_or(Rect::ZERO)));
    let marker_x = create_memo(clone!(marker_rect -> move || marker_rect.get().min.x));
    let marker_y = create_memo(clone!(marker_rect -> move || marker_rect.get().min.y));
    let marker_width = create_memo(clone!(marker_rect -> move || marker_rect.get().width()));
    let marker_height = create_memo(clone!(marker_rect -> move || marker_rect.get().height()));
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
                            <DockPanelView dock surface leaf hoisted />
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
                            <DockSplitterView dock surface split />
                        </CanvasItem>
                    }
                }}
            </ForEach>
            <CanvasItem x={marker_x} y={marker_y} width={marker_width} height={marker_height}>
                <Frame visible={marker_shown}>{marked.highlight.call(())}</Frame>
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn DockPanelView(dock: Handle, surface: SurfaceId, leaf: LeafId, hoisted: bool) -> NodeId {
    let state = dock.state.clone();
    let focused = create_memo(clone!(state -> move || {
        state.with(|state| state.focused_leaf() == Some(leaf))
    }));
    let floating = state.with_untracked(|state| state.window_rect(surface).is_some());
    let float = ClickCallback::new(clone!(dock -> move || {
        if let Some(tab) = dock.state.get_untracked().active_tab(leaf) {
            dock.float_tab(tab);
        }
    }));
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
        focused,
        bar,
        body,
        float,
    });
    let pressed = dock.clone();
    let hovered = dock.clone();
    view! {
        <ClickCatcher
            on_press={move |_: PointerPress| pressed.edit(|state| state.focus(leaf))}
            on_hover_move={move |press: PointerPress| hovered.drag_to(press.pos, press.modifiers.alt)}
            children={chrome}
        />
    }
}

#[component]
fn DockPanelBody(dock: Handle, leaf: LeafId) -> NodeId {
    let state = dock.state.clone();
    let tabs = create_memo(clone!(state -> move || state.with(|state| state.tabs(leaf))));
    let shown = create_memo(clone!(state -> move || state.with(|state| state.active_tab(leaf))));
    let content = dock.content.clone();
    view! {
        <List spacing=0.0>
            <ForEach keys={tabs}>
                {move |tab: TabId| {
                    let content = content.clone();
                    let visible = create_memo(clone!(shown -> move || shown.get() == Some(tab)));
                    let sizing = create_memo(clone!(visible -> move || match visible.get() {
                        true => ItemSize::Percent(100.0),
                        false => ItemSize::Fixed(0.0),
                    }));
                    view! {
                        <Frame @sizing={sizing} visible={visible}>{content.call(tab)}</Frame>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn DockTabBar(dock: Handle, leaf: LeafId) -> NodeId {
    let bar = NodeRef::new();
    dock.bars.borrow_mut().insert(leaf, bar.clone());
    on_cleanup(clone!(dock -> move || {
        dock.bars.borrow_mut().remove(&leaf);
    }));
    let state = dock.state.clone();
    let tabs = create_memo(clone!(state -> move || state.with(|state| state.tabs(leaf))));
    let selected = create_memo(clone!(state -> move || {
        Some(state.with(|state| state.active_index(leaf)))
    }));
    let labels = dock.clone();
    let options = view! {
        <ForEach keys={tabs}>
            {move |tab: TabId| {
                let dock = labels.clone();
                let label = create_memo(move || dock.title(tab));
                view! {
                    <ChoiceOption label={label} />
                }
            }}
        </ForEach>
    };
    let changed = dock.clone();
    let faces = dock.clone();
    view! {
        <Choice
            @node_ref=&bar
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
                let tab = dock.state.get_untracked().tabs(leaf).get(handle.index).copied();
                match tab {
                    Some(tab) => view! {
                        <DockTabView dock leaf tab handle />
                    },
                    None => view! {
                        <Frame />
                    },
                }
            }}
        </Choice>
    }
}

#[component]
fn DockTabView(dock: Handle, leaf: LeafId, tab: TabId, handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        index,
        selected,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let title = create_memo(clone!(dock -> move || dock.title(tab)));
    let dragged = create_memo(clone!(dock -> move || {
        dock.drag
            .with(|drag| drag.as_ref().is_some_and(|drag| drag.tab == tab && drag.moved))
    }));
    let close = ClickCallback::new(clone!(dock -> move || dock.close_tab(tab)));
    let face = dock.tab.call(DockTabHandle {
        tab,
        leaf,
        index,
        title,
        selected,
        hovered,
        active,
        focused,
        dragged,
        close,
    });
    let pressed = dock.clone();
    let moved = dock.clone();
    let released = dock.clone();
    view! {
        <ClickCatcher
            on_press={move |press: PointerPress| pressed.begin_drag(tab, press.pos)}
            on_drag={move |press: PointerPress| moved.drag_to(press.pos, press.modifiers.alt)}
            on_active_change={move |active: bool| {
                if !active {
                    released.end_drag();
                }
            }}
            children={face}
        />
    }
}

#[component]
fn DockSplitterView(dock: Handle, surface: SurfaceId, split: SplitId) -> NodeId {
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
                on_drag={move |press: PointerPress| {
                    let Some(splitter) = dragged.splitter_of(surface, split) else {
                        return;
                    };
                    let fraction = fraction_at(
                        splitter.area,
                        splitter.direction,
                        press.pos,
                        dragged.thickness,
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
    let state = dock.state.clone();
    let rect = create_memo(clone!(state -> move || {
        state.with(|state| state.window_rect(surface)).unwrap_or(Rect::ZERO)
    }));
    let area = dock.rect.clone();
    let anchor =
        create_memo(clone!(rect area -> move || area.get().min + rect.get().min.to_vec2()))
            .into_prop()
            .map(OverlayAnchor::Point);
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
                    .and_then(|leaf| state.active_tab(*leaf))
            })
            .map(|tab| dock.title(tab))
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
    let grabbed: Rc<Cell<(Rect, Pos2)>> = Rc::new(Cell::new((Rect::ZERO, Pos2::ZERO)));
    let bar_rect = rect.clone();
    let pressed = dock.clone();
    let start = grabbed.clone();
    let moved = dock.clone();
    let grip = view! {
        <ClickCatcher
            cursor=CursorIcon::Grab
            on_press={move |press: PointerPress| {
                start.set((bar_rect.get_untracked(), press.pos));
                pressed.edit(|state| {
                    if let Some(leaf) = state.leaves(surface).first().copied() {
                        state.focus(leaf);
                    }
                });
            }}
            on_drag={move |press: PointerPress| {
                let (start, from) = grabbed.get();
                let placed = Rect::from_min_size(start.min + (press.pos - from), start.size());
                let bounds = moved.rect.get_untracked().size();
                let origin = moved.clamped_origin(placed, bounds);
                moved.edit(|state| {
                    state.set_window_rect(surface, Rect::from_min_size(origin, start.size()));
                });
            }}
            children={grip_face}
        />
    };
    let hoisted = state.with_untracked(|state| state.leaves(surface).first().copied());
    let tabs = hoisted.map(|leaf| {
        view! {
            <DockTabBar dock={dock.clone()} leaf />
        }
    });
    let grips = dock.clone();
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
        <DockPane dock={dock.clone()} surface hoisted />
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
    view! {
        <Overlay
            @node_ref=&overlay
            anchor={anchor}
            placement=Placement::BelowStart
            mode=OverlayMode::Floating
            traps_focus=false
            open=true
        >
            <Frame width={width.clone()} height={height.clone()}>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>{chrome}</CanvasItem>
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
    let dragging = create_memo(clone!(drag -> move || {
        drag.with(|drag| drag.as_ref().is_some_and(|drag| drag.moved))
    }));
    let marked = create_memo(clone!(drag -> move || {
        drag.with(|drag| {
            let drag = drag.as_ref()?;
            matches!(drag.target?, DropTarget::Window { .. })
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
    let preview_anchor = create_memo(clone!(drag -> move || {
        drag.with(|drag| drag.as_ref().map_or(Pos2::ZERO, |drag| drag.pointer + PREVIEW_OFFSET))
    }))
    .into_prop()
    .map(OverlayAnchor::Point);
    let dragged =
        create_memo(clone!(drag -> move || drag.with(|drag| drag.as_ref().map(|drag| drag.tab))));
    let face = dock.highlight.call(());
    let preview = dock.preview.clone();
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
            <Overlay
                anchor={preview_anchor}
                placement=Placement::BelowStart
                mode=OverlayMode::Passive
                traps_focus=false
                open={dragging}
            >
                <List spacing=0.0>
                    <Dynamic value={dragged}>
                        {move |tab: Option<TabId>| match tab {
                            Some(tab) => preview.call(tab),
                            None => view! {
                                <Frame />
                            },
                        }}
                    </Dynamic>
                </List>
            </Overlay>
        </List>
    }
}

pub fn dock_state(document: &Document, dock: NodeId) -> DockState {
    document.component_state::<Handle>(dock).state.get()
}
