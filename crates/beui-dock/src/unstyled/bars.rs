use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui_macros::{component, view};
use beui_view::actions::{Action, MenuNode};
use beui_view::components::overlay::Overlay;
use beui_view::components::root::Root;
use beui_view::drag::{DragPoint, drag_board};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::window::{WindowPlacement, WindowReport};
use beui_view::types::{
    Align, Align2, Color, Direction, ItemSize, LayerPolicy, Placement, PointerPress,
};
use beui_view::types::{Pos2, Rect, Sides, Vec2, pos2, vec2};
use beui_view::window::{Window, WindowHandle};

use super::{
    DockBarMove, DockBarPlacement, DockDragStage, DockDragged, DockEdge, DockHosted, DockMode,
    DockRequest, FLOAT_INSET, LeafId, State, TabId,
};

const BAND_SHARE: f32 = 0.12;
const BAND_MIN: f32 = 24.0;
const BAND_MAX: f32 = 56.0;
const CHIP_LENGTH: f32 = 96.0;
const CHIP_SHARE: f32 = 0.3;
const CHIP_DEPTH: f32 = 0.5;
const WINDOW_ROOM: f32 = 4096.0;
const WINDOW_SIZE: Vec2 = Vec2::new(240.0, 40.0);
const POP_OFFSET: Vec2 = Vec2::new(16.0, 16.0);

const CHIPS: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

const TARGETS: [DockBarPlacement; 8] = [
    DockBarPlacement::Beside(DockEdge::Top),
    DockBarPlacement::Over(DockEdge::Top),
    DockBarPlacement::Beside(DockEdge::Bottom),
    DockBarPlacement::Over(DockEdge::Bottom),
    DockBarPlacement::Beside(DockEdge::Left),
    DockBarPlacement::Over(DockEdge::Left),
    DockBarPlacement::Beside(DockEdge::Right),
    DockBarPlacement::Over(DockEdge::Right),
];

pub struct DockBarTargetHandle {
    pub placement: DockBarPlacement,
    pub active: Memo<bool>,
}

#[derive(Clone, Copy)]
pub struct DockBarControl {
    bar: TabId,
    backing: BarBacking,
    menu: Memo<Vec<MenuNode>>,
}

#[derive(Clone, Copy)]
enum BarBacking {
    Local(State),
    Hosted {
        hosted: Memo<Option<DockHosted>>,
        request: Callback<DockRequest>,
        last: Signal<Option<Pos2>>,
    },
}

impl DockBarControl {
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
        Self {
            bar,
            backing: BarBacking::Hosted {
                hosted,
                request,
                last: create_signal(None),
            },
            menu: bar_menu(placement, place),
        }
    }

    pub fn bar(&self) -> TabId {
        self.bar
    }

    pub fn is_hosted(&self) -> bool {
        matches!(self.backing, BarBacking::Hosted { .. })
    }

    pub fn placement(&self) -> DockBarPlacement {
        let bar = self.bar;
        let placement = match self.backing {
            BarBacking::Local(dock) => dock.state.with(|state| state.bar_placement(bar)),
            BarBacking::Hosted { hosted, .. } => {
                hosted.with(|hosted| hosted.as_ref()?.state.bar_placement(bar))
            }
        };
        placement.unwrap_or(DockBarPlacement::Over(DockEdge::Top))
    }

    pub fn edge(&self) -> DockEdge {
        self.placement().edge()
    }

    pub fn direction(&self) -> Direction {
        self.placement().direction()
    }

    pub fn menu(&self) -> Memo<Vec<MenuNode>> {
        self.menu
    }

    pub fn moving(&self) -> bool {
        let bar = self.bar;
        match self.backing {
            BarBacking::Local(dock) => dock.drag.with(|drag| {
                drag.as_ref()
                    .is_some_and(|drag| drag.dragged == DockDragged::Bar(bar))
            }),
            BarBacking::Hosted { last, .. } => last.with(Option::is_some),
        }
    }

    pub fn place(&self, placement: DockBarPlacement) {
        match self.backing {
            BarBacking::Local(dock) => dock.place_bar(self.bar, placement),
            BarBacking::Hosted { request, .. } => {
                request.call(DockRequest::PlaceBar(self.bar, Some(placement)));
            }
        }
    }

    pub fn pop_out(&self) {
        match self.backing {
            BarBacking::Local(dock) => dock.pop_out_bar(self.bar),
            BarBacking::Hosted { request, .. } => {
                request.call(DockRequest::PlaceBar(self.bar, None));
            }
        }
    }

    pub fn begin_move(&self, press: PointerPress) {
        let dock = match self.backing {
            BarBacking::Local(dock) => dock,
            BarBacking::Hosted { request, last, .. } => {
                last.set(Some(press.pos));
                request.call(DockRequest::MoveBar(
                    self.bar,
                    DockBarMove {
                        at: press.pos,
                        stage: DockDragStage::Move,
                    },
                ));
                return;
            }
        };
        if !dock.panels.is_alive() || dock.mode.get_untracked() != DockMode::Tiled {
            return;
        }
        let bar = self.bar;
        if untrack(|| self.moving()) {
            drag_board().track(DragPoint::of(press));
            return;
        }
        let windowed = untrack(|| self.placement().is_window());
        if windowed && dock.opens.get_untracked() {
            let handle = dock.bar_windows.value().borrow().get(&bar).copied();
            if handle.is_some_and(WindowHandle::begin_move) {
                return;
            }
        }
        let rect = dock.bar_panel_rect(bar);
        let grab = rect.map_or(Vec2::ZERO, |rect| press.pos - rect.min);
        let size = rect.map_or(WINDOW_SIZE, |rect| rect.size());
        dock.bar_grab.set((grab, size));
        dock.begin_drag(DockDragged::Bar(bar));
        drag_board().begin(
            Rc::new(DockDragged::Bar(bar)),
            DragPoint::of(press),
            Rc::new(|_| {}),
            Rc::new(move |point: DragPoint| dock.bar_outside(bar, point)),
        );
    }

    pub fn end_move(&self) {
        let BarBacking::Hosted { request, last, .. } = self.backing else {
            return;
        };
        let Some(at) = last.get_untracked() else {
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

impl State {
    pub(super) fn bar_panel(&self, bar: TabId) -> NodeId {
        if let Some((panel, _)) = self.bar_panels.value().borrow().get(&bar) {
            return *panel;
        }
        let scope = self.open_scope();
        let dock = *self;
        let panel = scope.context().run(|| {
            let state = dock.state;
            let placement = create_memo(move || state.with(|state| state.bar_placement(bar)));
            let place = Callback::new(move |placement: Option<DockBarPlacement>| match placement {
                Some(placement) => dock.place_bar(bar, placement),
                None => dock.pop_out_bar(bar),
            });
            provide_context(DockBarControl {
                bar,
                backing: BarBacking::Local(dock),
                menu: bar_menu(placement, place),
            });
            self.content.call(bar)
        });
        self.bar_panels
            .value()
            .borrow_mut()
            .insert(bar, (panel, scope));
        panel
    }

    fn place_bar(&self, bar: TabId, placement: DockBarPlacement) {
        if placement.is_window() {
            self.bar_placements
                .update(|placements| placements.retain(|held, _| *held != bar));
        }
        self.edit(|state| state.set_bar_placement(bar, placement));
    }

    fn pop_out_bar(&self, bar: TabId) {
        let dock = self.rect.get_untracked();
        let rect = self.bar_panel_rect(bar);
        let origin = rect.map_or(pos2(FLOAT_INSET.x, FLOAT_INSET.y), |rect| {
            rect.min - dock.min.to_vec2() + POP_OFFSET
        });
        let size = rect.map_or(WINDOW_SIZE, |rect| rect.size());
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        let edge = self
            .state
            .with_untracked(|state| state.bar_placement(bar))
            .map_or(DockEdge::Top, DockBarPlacement::edge);
        self.place_bar(bar, DockBarPlacement::Window { edge, origin });
    }

    pub(super) fn ask_bar(&self, request: DockRequest) {
        match request {
            DockRequest::PlaceBar(bar, Some(placement)) => self.place_bar(bar, placement),
            DockRequest::PlaceBar(bar, None) => self.pop_out_bar(bar),
            DockRequest::MoveBar(bar, moved) => self.carry_bar(bar, moved),
            DockRequest::Show(_) | DockRequest::Restore(_) | DockRequest::Fullscreen(_) => {}
        }
    }

    fn carry_bar(&self, bar: TabId, moved: DockBarMove) {
        let carried = self.drag.with_untracked(|drag| {
            drag.as_ref()
                .is_some_and(|drag| drag.dragged == DockDragged::Bar(bar))
        });
        let rect = self.bar_panel_rect(bar);
        let point = rect.map(|rect| {
            let mut point = DragPoint::at(rect.min + moved.at.to_vec2(), Default::default());
            point.window = self.home();
            point
        });
        match moved.stage {
            DockDragStage::Move if carried => self.drag_over(point),
            DockDragStage::Move => {
                if self.mode.get_untracked() != DockMode::Tiled {
                    return;
                }
                let windowed = self
                    .state
                    .with_untracked(|state| state.bar_placement(bar))
                    .is_some_and(DockBarPlacement::is_window);
                if windowed && self.opens.get_untracked() {
                    let handle = self.bar_windows.value().borrow().get(&bar).copied();
                    if handle.is_some_and(WindowHandle::begin_move) {
                        return;
                    }
                }
                let Some(rect) = rect else {
                    return;
                };
                self.bar_grab.set((moved.at.to_vec2(), rect.size()));
                self.begin_drag(DockDragged::Bar(bar));
                self.drag_over(point);
            }
            DockDragStage::Drop if carried => {
                if let Some(point) = point {
                    self.drop_bar(bar, point);
                }
                self.end_drag();
            }
            DockDragStage::Cancel if carried => self.end_drag(),
            DockDragStage::Drop | DockDragStage::Cancel => {}
        }
    }

    pub(super) fn keep_bar_panels(&self, bars: &[TabId]) {
        let dropped: Vec<TabId> = self
            .bar_panels
            .value()
            .borrow()
            .keys()
            .filter(|bar| !bars.contains(bar))
            .copied()
            .collect();
        for bar in dropped {
            let panel = self.bar_panels.value().borrow_mut().remove(&bar);
            drop(panel);
        }
    }

    fn bar_panel_rect(&self, bar: TabId) -> Option<Rect> {
        let (panel, _) = *self.bar_panels.value().borrow().get(&bar)?;
        Some(measure(panel).get_untracked()?.rect)
    }

    pub(super) fn bar_body(&self, bar: TabId) -> Option<Rect> {
        let state = self.state.get_untracked();
        let owner = state.bar_owner(bar)?;
        let leaf = state.find(owner)?.leaf;
        if !state.is_shown(owner, false) {
            return None;
        }
        let node = self.bodies.value().borrow().get(&leaf)?.try_get()?;
        Some(measure(node).get_untracked()?.rect)
    }

    pub(super) fn resolve_bar(
        &self,
        point: DragPoint,
        bar: TabId,
    ) -> (Option<DockBarPlacement>, Option<Rect>) {
        let Some(current) = self.state.with_untracked(|state| state.bar_placement(bar)) else {
            return (None, None);
        };
        if point.window != self.home() {
            return (None, None);
        }
        let (grab, size) = self.bar_grab.get();
        if let Some(body) = self.bar_body(bar).filter(|body| body.contains(point.pos)) {
            return match bar_zone(body, point.pos) {
                Some(placement) => (
                    Some(placement),
                    Some(bar_highlight(body, placement, size, self.bar_margin)),
                ),
                None => (None, None),
            };
        }
        let dock = self.rect.get_untracked();
        let origin = point.pos - dock.min.to_vec2() - grab;
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        (
            Some(DockBarPlacement::Window {
                edge: current.edge(),
                origin,
            }),
            Some(Rect::from_min_size(dock.min + origin.to_vec2(), size)),
        )
    }

    pub(super) fn drop_bar(&self, bar: TabId, point: DragPoint) {
        let (target, _) = self.resolve_bar(point, bar);
        let Some(placement) = target else {
            return;
        };
        if placement.is_window() {
            self.bar_placements
                .update(|placements| placements.retain(|held, _| *held != bar));
        }
        self.edit(|state| state.set_bar_placement(bar, placement));
    }

    fn bar_outside(&self, bar: TabId, _: DragPoint) {
        if !self.opens.get_untracked() {
            return;
        }
        let edge = self
            .state
            .with_untracked(|state| state.bar_placement(bar))
            .map_or(DockEdge::Top, DockBarPlacement::edge);
        let origin = pos2(FLOAT_INSET.x, FLOAT_INSET.y);
        self.edit(|state| state.set_bar_placement(bar, DockBarPlacement::Window { edge, origin }));
        self.bar_placements.update(|placements| {
            placements.insert(bar, WindowPlacement::AtPointer);
        });
    }

    fn bars_at(
        &self,
        tab: Memo<Option<TabId>>,
        wanted: impl Fn(DockBarPlacement) -> bool + 'static,
    ) -> Memo<Vec<TabId>> {
        let state = self.state;
        create_memo(move || {
            let Some(tab) = tab.get() else {
                return Vec::new();
            };
            state.with(|state| {
                state
                    .bars_of(tab)
                    .into_iter()
                    .filter(|bar| state.bar_placement(*bar).is_some_and(&wanted))
                    .collect()
            })
        })
    }
}

fn bar_menu(
    placement: Memo<Option<DockBarPlacement>>,
    place: Callback<Option<DockBarPlacement>>,
) -> Memo<Vec<MenuNode>> {
    let item = move |target: DockBarPlacement, label: String| {
        let id = match target {
            DockBarPlacement::Over(edge) => format!("dock.bar.over.{}", edge.name()),
            DockBarPlacement::Beside(edge) => format!("dock.bar.beside.{}", edge.name()),
            DockBarPlacement::Window { .. } => "dock.bar.window".to_owned(),
        };
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
    for edge in DockEdge::ALL {
        nodes.push(item(
            DockBarPlacement::Over(edge),
            format!("Float over the {}", edge.name()),
        ));
    }
    nodes.push(MenuNode::Separator);
    for edge in DockEdge::ALL {
        nodes.push(item(
            DockBarPlacement::Beside(edge),
            format!("Dock along the {}", edge.name()),
        ));
    }
    nodes.push(MenuNode::Separator);
    nodes.push(item(
        DockBarPlacement::Window {
            edge: DockEdge::Top,
            origin: Pos2::ZERO,
        },
        "Pop out into a window".to_owned(),
    ));
    create_memo(move || nodes.clone())
}

fn band(body: Rect) -> f32 {
    (body.width().min(body.height()) * BAND_SHARE).clamp(BAND_MIN, BAND_MAX)
}

fn nearest_edge(body: Rect, pos: Pos2) -> (DockEdge, f32) {
    [
        (DockEdge::Top, pos.y - body.top()),
        (DockEdge::Bottom, body.bottom() - pos.y),
        (DockEdge::Left, pos.x - body.left()),
        (DockEdge::Right, body.right() - pos.x),
    ]
    .into_iter()
    .min_by(|(_, first), (_, second)| first.total_cmp(second))
    .unwrap_or((DockEdge::Top, 0.0))
}

pub(super) fn bar_zone(body: Rect, pos: Pos2) -> Option<DockBarPlacement> {
    let (edge, distance) = nearest_edge(body, pos);
    let band = band(body);
    if distance < band {
        return Some(DockBarPlacement::Beside(edge));
    }
    if distance < band * 2.0 {
        return Some(DockBarPlacement::Over(edge));
    }
    None
}

fn edge_rect(body: Rect, edge: DockEdge, depth: f32, along: f32, across: f32) -> Rect {
    let size = match edge.direction() {
        Direction::Horizontal => vec2(along, across),
        Direction::Vertical => vec2(across, along),
    };
    let center = match edge {
        DockEdge::Top => pos2(body.center().x, body.top() + depth),
        DockEdge::Bottom => pos2(body.center().x, body.bottom() - depth),
        DockEdge::Left => pos2(body.left() + depth, body.center().y),
        DockEdge::Right => pos2(body.right() - depth, body.center().y),
    };
    Rect::from_center_size(center, size)
}

pub(super) fn bar_chip(body: Rect, placement: DockBarPlacement) -> Rect {
    let band = band(body);
    let edge = placement.edge();
    let length = match edge.direction() {
        Direction::Horizontal => body.width(),
        Direction::Vertical => body.height(),
    };
    let depth = match placement {
        DockBarPlacement::Over(_) => band * (1.0 + CHIP_DEPTH),
        _ => band * CHIP_DEPTH,
    };
    let along = (length * CHIP_SHARE).min(CHIP_LENGTH);
    edge_rect(body, edge, depth, along, band * CHIP_DEPTH)
}

pub(super) fn bar_highlight(
    body: Rect,
    placement: DockBarPlacement,
    size: Vec2,
    margin: f32,
) -> Rect {
    let edge = placement.edge();
    let length = match edge.direction() {
        Direction::Horizontal => body.width(),
        Direction::Vertical => body.height(),
    };
    let thickness = size.x.min(size.y).max(1.0);
    match placement {
        DockBarPlacement::Over(_) => {
            let along = size.x.max(size.y).min((length - margin * 2.0).max(1.0));
            edge_rect(body, edge, margin + thickness / 2.0, along, thickness)
        }
        _ => edge_rect(body, edge, thickness / 2.0, length, thickness),
    }
}

fn across(edge: DockEdge) -> Direction {
    match edge.direction() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    }
}

fn float_align(edge: DockEdge) -> Align2 {
    match edge {
        DockEdge::Top => Align2::new(Align::Center, Align::Start),
        DockEdge::Bottom => Align2::new(Align::Center, Align::End),
        DockEdge::Left => Align2::new(Align::Start, Align::Center),
        DockEdge::Right => Align2::new(Align::End, Align::Center),
    }
}

#[component]
pub(super) fn DockBarred(
    dock: State,
    leaf: Option<LeafId>,
    tab: Memo<Option<TabId>>,
    stacked: bool,
    children: Child,
) -> NodeId {
    let body = NodeRef::new();
    if let Some(leaf) = leaf {
        dock.bodies.value().borrow_mut().insert(leaf, body);
        on_cleanup(move || {
            dock.bodies.try_with(|bodies| {
                let mut bodies = bodies.borrow_mut();
                if bodies
                    .get(&leaf)
                    .is_some_and(|held| held.try_get() == body.try_get())
                {
                    bodies.remove(&leaf);
                }
            });
        });
    }
    view! {
        <List direction=Direction::Vertical spacing=0.0>
            <DockBarBand dock tab edge=DockEdge::Top />
            <List direction=Direction::Horizontal spacing=0.0 @sizing=ItemSize::Percent(100.0)>
                <DockBarBand dock tab edge=DockEdge::Left />
                <Layers @node_ref=&body @sizing=ItemSize::Percent(100.0)>
                    {children}
                    <DockBarFloat dock tab edge=DockEdge::Top stacked />
                    <DockBarFloat dock tab edge=DockEdge::Bottom stacked />
                    <DockBarFloat dock tab edge=DockEdge::Left stacked />
                    <DockBarFloat dock tab edge=DockEdge::Right stacked />
                </Layers>
                <DockBarBand dock tab edge=DockEdge::Right />
            </List>
            <DockBarBand dock tab edge=DockEdge::Bottom />
        </List>
    }
}

#[component]
fn DockBarBand(dock: State, tab: Memo<Option<TabId>>, edge: DockEdge) -> NodeId {
    let bars = dock.bars_at(tab, move |placement| {
        placement == DockBarPlacement::Beside(edge)
    });
    view! {
        <List direction={across(edge)} spacing=0.0>
            <ForEach keys={bars}>|bar: TabId| {
                let dock = dock;
                view! {
                    <Portal node={Some(dock.bar_panel(bar))} />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn DockBarFloat(dock: State, tab: Memo<Option<TabId>>, edge: DockEdge, stacked: bool) -> NodeId {
    let bars = dock.bars_at(tab, move |placement| match placement {
        DockBarPlacement::Over(held) => held == edge,
        DockBarPlacement::Window { edge: held, .. } => stacked && held == edge,
        DockBarPlacement::Beside(_) => false,
    });
    let margin = dock.bar_margin;
    view! {
        <Frame align={float_align(edge)} padding={Sides::all(margin)}>
            <List direction={across(edge)} spacing={margin}>
                <ForEach keys={bars}>|bar: TabId| {
                    let dock = dock;
                    view! {
                        <Portal node={Some(dock.bar_panel(bar))} />
                    }
                }</ForEach>
            </List>
        </Frame>
    }
}

#[component]
pub(super) fn DockBarWindows(dock: State) -> NodeId {
    let state = dock.state;
    let bars = create_memo(move || {
        state.with(|state| {
            state
                .bars()
                .into_iter()
                .filter(|bar| {
                    state
                        .bar_placement(*bar)
                        .is_some_and(DockBarPlacement::is_window)
                        && state.is_bar_shown(*bar, false)
                })
                .collect::<Vec<_>>()
        })
    });
    view! {
        <List spacing=0.0>
            <ForEach keys={bars}>|bar: TabId| {
                let dock = dock;
                view! {
                    <DockBarWindow dock bar />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn DockBarWindow(dock: State, bar: TabId) -> NodeId {
    let state = dock.state;
    let panel = dock.bar_panel(bar);
    let measured = measure(panel);
    let size = create_signal(
        measured
            .get_untracked()
            .map(|measured| measured.size)
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(WINDOW_SIZE),
    );
    create_effect(move || {
        let Some(measured) = measured.get() else {
            return;
        };
        if measured.size.x > 0.0 && measured.size.y > 0.0 && size.get_untracked() != measured.size {
            size.set(measured.size);
        }
    });
    let origin = create_memo(move || {
        let origin = state.with(|state| match state.bar_placement(bar) {
            Some(DockBarPlacement::Window { origin, .. }) => origin,
            _ => pos2(FLOAT_INSET.x, FLOAT_INSET.y),
        });
        dock.clamped_origin(
            Rect::from_min_size(origin, size.get()),
            dock.rect.get().size(),
        )
    });
    let at = create_memo(move || dock.rect.get().min + origin.get().to_vec2());
    let parent = dock.home();
    let placement = create_memo(move || {
        dock.bar_placements
            .with(|placements| placements.get(&bar).cloned())
            .unwrap_or(WindowPlacement::Over {
                window: parent,
                at: at.get(),
            })
    });
    let overlay = NodeRef::new();
    let windows = dock.bar_windows;
    on_cleanup(move || {
        windows.try_with(|windows| windows.borrow_mut().remove(&bar));
    });
    let view = view! {
        <Window
            @node_ref=&overlay
            id={None}
            size={size}
            at={at}
            placement={placement}
            on_report={move |report: WindowReport| match report {
                WindowReport::Placed { placement, .. } => dock.bar_placements.update(|placements| {
                    placements.insert(bar, WindowPlacement::Host(placement));
                }),
                WindowReport::Closed(_) => {
                    let edge = dock
                        .state
                        .with_untracked(|state| state.bar_placement(bar))
                        .map_or(DockEdge::Top, DockBarPlacement::edge);
                    dock.edit(|state| state.set_bar_placement(bar, DockBarPlacement::Over(edge)));
                }
                _ => {}
            }}
        >|handle: WindowHandle| {
            windows.value().borrow_mut().insert(bar, handle);
            view! {
                <Root clear_color=Color::TRANSPARENT>
                    <Canvas>
                        <CanvasItem x=0.0 y=0.0 width=WINDOW_ROOM height=WINDOW_ROOM clip=false>
                            <Frame align=Align2::START>
                                <Portal node={Some(panel)} />
                            </Frame>
                        </CanvasItem>
                    </Canvas>
                </Root>
            }
            .child()
        }</Window>
    };
    create_effect(move || untrack(|| dock.raise_window(overlay)));
    view
}

#[component]
pub(super) fn DockBarTargets(dock: State) -> NodeId {
    let drag = dock.drag;
    let dragged = create_memo(move || {
        drag.with(|drag| match drag.as_ref()?.dragged {
            DockDragged::Bar(bar) => Some(bar),
            _ => None,
        })
    });
    let body = create_memo(move || dragged.get().and_then(|bar| dock.bar_body(bar)));
    let open = create_memo(move || body.get().is_some());
    let shown = create_memo(move || body.get().unwrap_or(Rect::ZERO));
    let anchor = create_memo(move || shown.get().min)
        .into_prop()
        .map(OverlayAnchor::Point);
    let width = create_memo(move || shown.get().width());
    let height = create_memo(move || shown.get().height());
    view! {
        <List spacing=0.0>
            <Overlay
                anchor={anchor}
                placement=Placement::BelowStart
                policy=LayerPolicy::Inert
                traps_focus=false
                open={open}
            >
                <Frame width=width height=height>
                    <Canvas>
                        <ForEach keys={CHIPS.to_vec()}>|index: usize| {
                            let dock = dock;
                            let placement = TARGETS[index];
                            let rect = create_memo(move || {
                                let body = shown.get();
                                bar_chip(body, placement).translate(-body.min.to_vec2())
                            });
                            let x = create_memo(move || rect.get().min.x);
                            let y = create_memo(move || rect.get().min.y);
                            let width = create_memo(move || rect.get().width());
                            let height = create_memo(move || rect.get().height());
                            let active = create_memo(move || {
                                dock.drag.with(|drag| {
                                    drag.as_ref().and_then(|drag| drag.bar_target) == Some(placement)
                                })
                            });
                            view! {
                                <CanvasItem x={x} y={y} width={width} height={height}>
                                    {dock.bar_target.call(DockBarTargetHandle { placement, active })}
                                </CanvasItem>
                            }
                        }</ForEach>
                    </Canvas>
                </Frame>
            </Overlay>
        </List>
    }
}

pub(super) type BarPanels = RefCell<HashMap<TabId, (NodeId, Scope)>>;
