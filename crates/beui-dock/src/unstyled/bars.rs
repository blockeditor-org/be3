use stable_hash::HashMap;
use std::cell::RefCell;
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
use beui_view::types::window::{WindowId, WindowPlacement, WindowReport};
use beui_view::types::{
    Align, Align2, Color, Direction, ItemSize, LayerLevel, LayerPolicy, Placement, PointerPress,
};
use beui_view::types::{Pos2, Rect, Sides, Vec2, pos2, vec2};
use beui_view::window::{Window, WindowHandle};

use super::state::DockState;
use super::{
    DockAlign, DockBarMove, DockBarPlacement, DockDragStage, DockDragged, DockEdge, DockHosted,
    DockMode, DockRequest, FLOAT_INSET, LeafId, State, TabId,
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

const TARGETS: [DockBarPlacement; 24] = targets();

const fn targets() -> [DockBarPlacement; 24] {
    let mut targets = [DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center); 24];
    let mut edge = 0;
    while edge < 4 {
        let held = DockEdge::ALL[edge];
        let mut align = 0;
        while align < 3 {
            let at = DockAlign::ALL[align];
            targets[edge * 6 + align] = DockBarPlacement::Beside(held, at);
            targets[edge * 6 + 3 + align] = DockBarPlacement::Over(held, at);
            align += 1;
        }
        edge += 1;
    }
    targets
}

pub struct DockBarBandHandle {
    pub edge: DockEdge,
    pub body: NodeId,
}

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
        last: Signal<Option<(WindowId, Pos2)>>,
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
        let hide = Callback::new(move |()| request.call(DockRequest::HideBar(bar, true)));
        Self {
            bar,
            backing: BarBacking::Hosted {
                hosted,
                request,
                last: create_signal(None),
            },
            menu: bar_menu(placement, place, hide),
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
        placement.unwrap_or(DockBarPlacement::Over(DockEdge::Top, DockAlign::Center))
    }

    pub fn edge(&self) -> DockEdge {
        self.placement().edge()
    }

    pub fn is_hidden(&self) -> bool {
        let bar = self.bar;
        match self.backing {
            BarBacking::Local(dock) => dock.state.with(|state| state.is_bar_hidden(bar)),
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
            BarBacking::Local(dock) => dock.hide_bar(self.bar, hidden),
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
                let home = last.with_untracked(|last| last.map(|(home, _)| home));
                let home = home.unwrap_or(press.window);
                let Some(at) = window_point(press.pos, press.window, home) else {
                    return;
                };
                last.set(Some((home, at)));
                request.call(DockRequest::MoveBar(
                    self.bar,
                    DockBarMove {
                        at,
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
            let key = dock.window_key(bar);
            let handle = dock.bar_windows.value().borrow().get(&key).copied();
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
        let Some((_, at)) = last.get_untracked() else {
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
            let hide = Callback::new(move |()| dock.hide_bar(bar, true));
            provide_context(DockBarControl {
                bar,
                backing: BarBacking::Local(dock),
                menu: bar_menu(placement, place, hide),
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
            let key = self.window_key(bar);
            self.bar_placements.update(|placements| {
                placements.remove(&key);
            });
        }
        let dock = *self;
        self.edit(|state| {
            state.set_bar_placement(bar, placement);
            dock.share_placement(state, bar);
        });
    }

    pub(super) fn bar_level(&self) -> LayerLevel {
        match self.fullscreen_in_front() {
            true => LayerLevel::Normal,
            false => LayerLevel::Raised,
        }
    }

    fn hide_bar(&self, bar: TabId, hidden: bool) {
        let dock = *self;
        self.edit(|state| {
            state.set_bar_hidden(bar, hidden);
            dock.share_placement(state, bar);
        });
    }

    pub(super) fn window_key(&self, bar: TabId) -> String {
        shared_key(self.bar_key.call(bar), bar)
    }

    fn share_placement(&self, state: &mut DockState, bar: TabId) {
        let dock = *self;
        state.share_bar_placement(bar, &|held| dock.window_key(held));
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
            DockRequest::HideBar(bar, hidden) => self.hide_bar(bar, hidden),
            DockRequest::Show(_) | DockRequest::Restore(_) | DockRequest::Fullscreen(_) => {}
        }
    }

    fn carry_bar(&self, bar: TabId, moved: DockBarMove) {
        let carried = self.drag.with_untracked(|drag| {
            drag.as_ref()
                .is_some_and(|drag| drag.dragged == DockDragged::Bar(bar))
        });
        let rect = self.bar_home_rect(bar);
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

    fn bar_home_rect(&self, bar: TabId) -> Option<Rect> {
        let rect = self.bar_panel_rect(bar)?;
        let placement = self
            .state
            .with_untracked(|state| state.bar_placement(bar))?;
        let DockBarPlacement::Window { origin, .. } = placement else {
            return Some(rect);
        };
        let key = self.window_key(bar);
        let window = self
            .bar_windows
            .value()
            .borrow()
            .get(&key)
            .map(|handle| handle.id);
        if let Some(min) = window.and_then(|window| window_point(rect.min, window, self.home())) {
            return Some(Rect::from_min_size(min, rect.size()));
        }
        let dock = self.rect.get_untracked();
        let origin = self.clamped_origin(Rect::from_min_size(origin, rect.size()), dock.size());
        Some(Rect::from_min_size(
            dock.min + origin.to_vec2(),
            rect.size(),
        ))
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
        self.bar_areas(bar).map(|(body, _)| body)
    }

    fn bar_areas(&self, bar: TabId) -> Option<(Rect, Rect)> {
        let state = self.state.get_untracked();
        let (body, pane) = match state.is_dock_bar(bar) {
            true => (self.dock_body, self.dock_pane),
            false => {
                let tab = state.bar_tab(bar)?;
                let leaf = state.find(tab)?.leaf;
                if !state.is_shown(tab, false) {
                    return None;
                }
                *self.bodies.value().borrow().get(&leaf)?
            }
        };
        let body = measure(body.try_get()?).get_untracked()?.rect;
        let pane = pane
            .try_get()
            .and_then(|pane| measure(pane).get_untracked())
            .map_or(body, |pane| pane.rect);
        Some((body, pane))
    }

    fn stacked_on(&self, bar: TabId, placement: DockBarPlacement) -> Vec<(TabId, Rect)> {
        let state = self.state.get_untracked();
        state
            .bars_with(bar)
            .into_iter()
            .filter(|other| *other != bar && state.bar_placement(*other) == Some(placement))
            .filter_map(|other| Some((other, self.bar_panel_rect(other)?)))
            .collect()
    }

    fn band_align(&self, bar: TabId, edge: DockEdge, pos: Pos2) -> Option<DockAlign> {
        let at = along_of(edge, pos);
        DockAlign::ALL.into_iter().find(|align| {
            let lined = self.stacked_on(bar, DockBarPlacement::Beside(edge, *align));
            let start = lined
                .iter()
                .map(|(_, rect)| along_start(edge, *rect))
                .reduce(f32::min);
            let end = lined
                .iter()
                .map(|(_, rect)| along_end(edge, *rect))
                .reduce(f32::max);
            start
                .zip(end)
                .is_some_and(|(start, end)| (start..=end).contains(&at))
        })
    }

    fn band_slot(
        &self,
        bar: TabId,
        placement: DockBarPlacement,
        pos: Pos2,
    ) -> (usize, Option<f32>) {
        let edge = placement.edge();
        let at = along_of(edge, pos);
        let own = self.layer_of(bar);
        let lined = self.stacked_on(bar, placement);
        let slot = lined
            .iter()
            .filter(|(other, rect)| {
                self.layer_of(*other) == own
                    && (along_start(edge, *rect) + along_end(edge, *rect)) / 2.0 < at
            })
            .count();
        let before = lined
            .iter()
            .filter(|(other, _)| self.layer_of(*other) > own)
            .count()
            + slot;
        let marker = match (lined.get(before), lined.last()) {
            (Some((_, rect)), _) => Some(along_start(edge, *rect)),
            (None, Some((_, rect))) => Some(along_end(edge, *rect)),
            (None, None) => None,
        };
        (slot, marker)
    }

    fn layer_of(&self, bar: TabId) -> usize {
        self.state
            .with_untracked(|state| state.bar_depth(bar))
            .unwrap_or(0)
    }

    pub(super) fn resolve_bar(
        &self,
        point: DragPoint,
        bar: TabId,
    ) -> (Option<(DockBarPlacement, usize)>, Option<Rect>) {
        let Some(current) = self.state.with_untracked(|state| state.bar_placement(bar)) else {
            return (None, None);
        };
        if point.window != self.home() {
            return (None, None);
        }
        let (grab, size) = self.bar_grab.get();
        if let Some((body, pane)) = self
            .bar_areas(bar)
            .filter(|(_, pane)| pane.contains(point.pos))
        {
            let (edge, distance) = nearest_edge(pane, point.pos);
            let beside = reach(pane, body, edge);
            let band = band(body);
            if distance < beside + band {
                let align = self
                    .band_align(bar, edge, point.pos)
                    .unwrap_or_else(|| along_third(body, edge, point.pos));
                let placement = DockBarPlacement::Beside(edge, align);
                let (slot, marker) = self.band_slot(bar, placement, point.pos);
                let highlight = band_highlight(pane, body, placement, size, marker);
                return (Some((placement, slot)), Some(highlight));
            }
            let align = along_third(body, edge, point.pos);
            let placement = DockBarPlacement::Over(edge, align);
            let stacked = self.stacked_on(bar, placement);
            let over_far = stacked
                .iter()
                .map(|(_, rect)| reach_far(pane, *rect, edge))
                .fold(0.0_f32, f32::max);
            if distance >= (beside + band * 2.0).max(over_far + band) {
                return (None, None);
            }
            let own = self.layer_of(bar);
            let slot = stacked
                .iter()
                .filter(|(other, rect)| {
                    self.layer_of(*other) == own && centre_reach(pane, *rect, edge) < distance
                })
                .count();
            let before = stacked
                .iter()
                .filter(|(other, _)| self.layer_of(*other) > own)
                .count()
                + slot;
            let boundary = match (stacked.get(before), stacked.last()) {
                (Some((_, rect)), _) => reach(pane, *rect, edge),
                (None, Some((_, rect))) => reach_far(pane, *rect, edge),
                (None, None) => beside + self.bar_margin,
            };
            let highlight = slot_highlight(pane, body, placement, size, self.bar_margin, boundary);
            return (Some((placement, slot)), Some(highlight));
        }
        let dock = self.rect.get_untracked();
        let origin = point.pos - dock.min.to_vec2() - grab;
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        (
            Some((
                DockBarPlacement::Window {
                    edge: current.edge(),
                    origin,
                },
                0,
            )),
            Some(Rect::from_min_size(dock.min + origin.to_vec2(), size)),
        )
    }

    pub(super) fn drop_bar(&self, bar: TabId, point: DragPoint) {
        let (target, _) = self.resolve_bar(point, bar);
        let Some((placement, slot)) = target else {
            return;
        };
        if placement.is_window() {
            self.place_bar(bar, placement);
            return;
        }
        let dock = *self;
        self.edit(|state| {
            state.place_bar_at(bar, placement, slot);
            dock.share_placement(state, bar);
        });
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
        let dock = *self;
        self.edit(|state| {
            state.set_bar_placement(bar, DockBarPlacement::Window { edge, origin });
            dock.share_placement(state, bar);
        });
        let key = self.window_key(bar);
        self.bar_placements.update(|placements| {
            placements.insert(key, WindowPlacement::AtPointer);
        });
    }

    fn bars_at(
        &self,
        holder: BarHolder,
        edge: DockEdge,
        stacked: bool,
        wanted: impl Fn(DockBarPlacement) -> bool + 'static,
    ) -> Memo<Vec<TabId>> {
        let state = self.state;
        let away = self.bars_away;
        create_memo(move || {
            let held = match holder {
                BarHolder::Tab { tab, .. } => {
                    let Some(tab) = tab.get().filter(|_| !away.get()) else {
                        return Vec::new();
                    };
                    state.with(|state| state.pane_bars(tab))
                }
                BarHolder::Dock => state.with(|state| state.bars_of(None)),
            };
            let mut bars: Vec<TabId> = state.with(|state| {
                held.into_iter()
                    .filter(|bar| !state.is_bar_hidden(*bar))
                    .filter(|bar| state.bar_placement(*bar).is_some_and(&wanted))
                    .collect()
            });
            if stacked && matches!(edge, DockEdge::Bottom | DockEdge::Right) {
                bars.reverse();
            }
            bars
        })
    }
}

fn bar_menu(
    placement: Memo<Option<DockBarPlacement>>,
    place: Callback<Option<DockBarPlacement>>,
    hide: Callback<()>,
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
    let hidden = Action::new("dock.bar.hide", "Hide", move || hide.call(())).detached();
    nodes.push(MenuNode::action(&hidden));
    create_memo(move || nodes.clone())
}

fn band(body: Rect) -> f32 {
    (body.width().min(body.height()) * BAND_SHARE).clamp(BAND_MIN, BAND_MAX)
}

fn along_third(body: Rect, edge: DockEdge, pos: Pos2) -> DockAlign {
    let (start, length, at) = match edge.direction() {
        Direction::Horizontal => (body.left(), body.width(), pos.x),
        Direction::Vertical => (body.top(), body.height(), pos.y),
    };
    let share = (at - start) / length.max(1.0);
    if share < 1.0 / 3.0 {
        DockAlign::Start
    } else if share > 2.0 / 3.0 {
        DockAlign::End
    } else {
        DockAlign::Center
    }
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

fn reach(pane: Rect, rect: Rect, edge: DockEdge) -> f32 {
    match edge {
        DockEdge::Top => rect.top() - pane.top(),
        DockEdge::Bottom => pane.bottom() - rect.bottom(),
        DockEdge::Left => rect.left() - pane.left(),
        DockEdge::Right => pane.right() - rect.right(),
    }
}

fn reach_far(pane: Rect, rect: Rect, edge: DockEdge) -> f32 {
    match edge {
        DockEdge::Top => rect.bottom() - pane.top(),
        DockEdge::Bottom => pane.bottom() - rect.top(),
        DockEdge::Left => rect.right() - pane.left(),
        DockEdge::Right => pane.right() - rect.left(),
    }
}

fn along_of(edge: DockEdge, pos: Pos2) -> f32 {
    match edge.direction() {
        Direction::Horizontal => pos.x,
        Direction::Vertical => pos.y,
    }
}

fn along_start(edge: DockEdge, rect: Rect) -> f32 {
    along_of(edge, rect.min)
}

fn along_end(edge: DockEdge, rect: Rect) -> f32 {
    along_of(edge, rect.max)
}

fn band_highlight(
    pane: Rect,
    body: Rect,
    placement: DockBarPlacement,
    size: Vec2,
    marker: Option<f32>,
) -> Rect {
    let edge = placement.edge();
    let thickness = size.x.min(size.y).max(1.0);
    let length = size.x.max(size.y);
    let band = match edge {
        DockEdge::Top => Rect::from_min_max(pane.min, pos2(pane.right(), pane.top() + thickness)),
        DockEdge::Bottom => {
            Rect::from_min_max(pos2(pane.left(), pane.bottom() - thickness), pane.max)
        }
        DockEdge::Left => Rect::from_min_max(
            pos2(pane.left(), body.top()),
            pos2(pane.left() + thickness, body.bottom()),
        ),
        DockEdge::Right => Rect::from_min_max(
            pos2(pane.right() - thickness, body.top()),
            pos2(pane.right(), body.bottom()),
        ),
    };
    let length = length
        .min(along_end(edge, band) - along_start(edge, band))
        .max(1.0);
    let centred = edge_rect(band, edge, thickness / 2.0, length, thickness);
    let align = placement.align();
    let Some(marker) = marker else {
        return aligned(centred, band, edge, align, 0.0);
    };
    let start = match align {
        DockAlign::Start => marker,
        DockAlign::Center => marker - length / 2.0,
        DockAlign::End => marker - length,
    };
    let start = start
        .min(along_end(edge, band) - length)
        .max(along_start(edge, band));
    let shift = start - along_start(edge, centred);
    match edge.direction() {
        Direction::Horizontal => centred.translate(vec2(shift, 0.0)),
        Direction::Vertical => centred.translate(vec2(0.0, shift)),
    }
}

fn centre_reach(pane: Rect, rect: Rect, edge: DockEdge) -> f32 {
    (reach(pane, rect, edge) + reach_far(pane, rect, edge)) / 2.0
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

fn aligned(rect: Rect, body: Rect, edge: DockEdge, align: DockAlign, inset: f32) -> Rect {
    let shift = match (edge.direction(), align) {
        (_, DockAlign::Center) => return rect,
        (Direction::Horizontal, DockAlign::Start) => body.left() + inset - rect.left(),
        (Direction::Horizontal, DockAlign::End) => body.right() - inset - rect.right(),
        (Direction::Vertical, DockAlign::Start) => body.top() + inset - rect.top(),
        (Direction::Vertical, DockAlign::End) => body.bottom() - inset - rect.bottom(),
    };
    match edge.direction() {
        Direction::Horizontal => rect.translate(vec2(shift, 0.0)),
        Direction::Vertical => rect.translate(vec2(0.0, shift)),
    }
}

pub(super) fn bar_chip(body: Rect, placement: DockBarPlacement) -> Rect {
    let band = band(body);
    let edge = placement.edge();
    let length = match edge.direction() {
        Direction::Horizontal => body.width(),
        Direction::Vertical => body.height(),
    };
    let depth = match placement {
        DockBarPlacement::Over(..) => band * (1.0 + CHIP_DEPTH),
        _ => band * CHIP_DEPTH,
    };
    let along = (length * CHIP_SHARE).min(CHIP_LENGTH);
    let chip = edge_rect(body, edge, depth, along, band * CHIP_DEPTH);
    let inset = band * (1.0 + CHIP_DEPTH * 2.0);
    aligned(chip, body, edge, placement.align(), inset)
}

fn slot_highlight(
    pane: Rect,
    body: Rect,
    placement: DockBarPlacement,
    size: Vec2,
    margin: f32,
    boundary: f32,
) -> Rect {
    let edge = placement.edge();
    let length = match edge.direction() {
        Direction::Horizontal => body.width(),
        Direction::Vertical => body.height(),
    };
    let thickness = size.x.min(size.y).max(1.0);
    let along = size.x.max(size.y).min((length - margin * 2.0).max(1.0));
    let align = placement.align();
    let centre = match edge.direction() {
        Direction::Horizontal => pos2(body.center().x, pane.center().y),
        Direction::Vertical => pos2(pane.center().x, body.center().y),
    };
    let rect = edge_rect(
        Rect::from_center_size(centre, pane.size()),
        edge,
        boundary + thickness / 2.0,
        along,
        thickness,
    );
    aligned(rect, body, edge, align, margin)
}

fn across(edge: DockEdge) -> Direction {
    match edge.direction() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    }
}

fn float_align(edge: DockEdge, align: DockAlign) -> Align2 {
    let along = match align {
        DockAlign::Start => Align::Start,
        DockAlign::Center => Align::Center,
        DockAlign::End => Align::End,
    };
    match edge {
        DockEdge::Top => Align2::new(along, Align::Start),
        DockEdge::Bottom => Align2::new(along, Align::End),
        DockEdge::Left => Align2::new(Align::Start, along),
        DockEdge::Right => Align2::new(Align::End, along),
    }
}

#[component]
pub(super) fn DockBarred(
    dock: State,
    leaf: Option<LeafId>,
    holder: BarHolder,
    children: Child,
) -> NodeId {
    let (body, pane) = match holder {
        BarHolder::Dock => (dock.dock_body, dock.dock_pane),
        BarHolder::Tab { .. } => (NodeRef::new(), NodeRef::new()),
    };
    if let Some(leaf) = leaf {
        dock.bodies.value().borrow_mut().insert(leaf, (body, pane));
        on_cleanup(move || {
            dock.bodies.try_with(|bodies| {
                let mut bodies = bodies.borrow_mut();
                if bodies
                    .get(&leaf)
                    .is_some_and(|(held, _)| held.try_get() == body.try_get())
                {
                    bodies.remove(&leaf);
                }
            });
        });
    }
    let ends = [(); 4].map(|()| create_signal([0.0_f32; 2]));
    let [top, bottom, left, right] = ends;
    view! {
        <List @node_ref=&pane direction=Direction::Vertical spacing=0.0>
            <DockBarBand dock holder edge=DockEdge::Top />
            <List direction=Direction::Horizontal spacing=0.0 @sizing=ItemSize::Percent(100.0)>
                <DockBarBand dock holder edge=DockEdge::Left />
                <Layers @node_ref=&body @sizing=ItemSize::Percent(100.0)>
                    {children}
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Top
                        align=DockAlign::Start
                        ends={top}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Top
                        align=DockAlign::Center
                        ends={top}
                    />
                    <DockBarFloat dock holder edge=DockEdge::Top align=DockAlign::End ends={top} />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Bottom
                        align=DockAlign::Start
                        ends={bottom}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Bottom
                        align=DockAlign::Center
                        ends={bottom}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Bottom
                        align=DockAlign::End
                        ends={bottom}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Left
                        align=DockAlign::Start
                        ends={left}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Left
                        align=DockAlign::Center
                        ends={left}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Left
                        align=DockAlign::End
                        ends={left}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Right
                        align=DockAlign::Start
                        ends={right}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Right
                        align=DockAlign::Center
                        ends={right}
                    />
                    <DockBarFloat
                        dock
                        holder
                        edge=DockEdge::Right
                        align=DockAlign::End
                        ends={right}
                    />
                </Layers>
                <DockBarBand dock holder edge=DockEdge::Right />
            </List>
            <DockBarBand dock holder edge=DockEdge::Bottom />
        </List>
    }
}

#[component]
fn DockBarBand(dock: State, holder: BarHolder, edge: DockEdge) -> NodeId {
    let bars = dock.bars_at(holder, edge, false, move |placement| {
        placement.is_beside() && placement.edge() == edge
    });
    let shown = create_memo(move || match bars.with(Vec::is_empty) {
        true => Vec::new(),
        false => vec![edge],
    });
    view! {
        <List direction={across(edge)} spacing=0.0>
            <ForEach keys={shown}>|edge: DockEdge| {
                let dock = dock;
                let ends = create_signal([0.0_f32; 2]);
                let body = view! {
                    <Layers>
                        <DockBarRun
                            dock
                            holder
                            edge
                            placement={DockBarPlacement::Beside(edge, DockAlign::Start)}
                            ends
                        />
                        <DockBarRun
                            dock
                            holder
                            edge
                            placement={DockBarPlacement::Beside(edge, DockAlign::Center)}
                            ends
                        />
                        <DockBarRun
                            dock
                            holder
                            edge
                            placement={DockBarPlacement::Beside(edge, DockAlign::End)}
                            ends
                        />
                    </Layers>
                };
                dock.bar_band.call(DockBarBandHandle { edge, body })
            }</ForEach>
        </List>
    }
}

#[component]
fn DockBarRun(
    dock: State,
    holder: BarHolder,
    edge: DockEdge,
    placement: DockBarPlacement,
    ends: Signal<[f32; 2]>,
) -> NodeId {
    let bars = dock.bars_at(holder, edge, false, move |held| held == placement);
    view! {
        <DockBarLine
            dock
            bars
            edge
            align={placement.align()}
            direction={edge.direction()}
            margin=0.0
            ends
            held={NodeRef::new()}
        />
    }
}

#[component]
fn DockBarLine(
    dock: State,
    bars: Memo<Vec<TabId>>,
    edge: DockEdge,
    align: DockAlign,
    direction: Direction,
    margin: f32,
    ends: Signal<[f32; 2]>,
    held: NodeRef,
) -> NodeId {
    let padding = create_signal(Sides::all(margin));
    let built = view! {
        <Frame align={float_align(edge, align)} padding={padding}>
            <List @node_ref=&held direction={direction} spacing={margin}>
                <ForEach keys={bars}>|bar: TabId| {
                    let dock = dock;
                    view! {
                        <Portal
                            @sizing={ItemSize::Intrinsic.shrink(1.0)}
                            node={Some(dock.bar_panel(bar))}
                        />
                    }
                }</ForEach>
            </List>
        </Frame>
    };
    let Some(list) = held.try_get() else {
        return built;
    };
    let area = measure(built);
    let covered = measure(list);
    let along = move |size: Vec2| match edge.direction() {
        Direction::Horizontal => size.x,
        Direction::Vertical => size.y,
    };
    match align {
        DockAlign::Center => create_effect(move || {
            let [start, end] = ends.get();
            let (Some(area), Some(own)) = (area.get(), covered.get()) else {
                return;
            };
            let room = along(area.rect.size()) - margin * 2.0;
            let length = along(own.rect.size());
            let centred = (room - length) / 2.0;
            let at = centred.min(room - end - length).max(start);
            let shift = (at - centred) * 2.0;
            let mut sides = Sides::all(margin);
            let squeezed = start + length + end >= room - 0.5;
            match (edge.direction(), squeezed, shift >= 0.0) {
                (Direction::Horizontal, true, _) => {
                    sides.left += start;
                    sides.right += end;
                }
                (Direction::Vertical, true, _) => {
                    sides.top += start;
                    sides.bottom += end;
                }
                (Direction::Horizontal, false, true) => sides.left += shift,
                (Direction::Horizontal, false, false) => sides.right -= shift,
                (Direction::Vertical, false, true) => sides.top += shift,
                (Direction::Vertical, false, false) => sides.bottom -= shift,
            }
            if padding.get_untracked() != sides {
                padding.set(sides);
            }
        }),
        DockAlign::Start | DockAlign::End => create_effect(move || {
            let length = covered.get().map_or(0.0, |own| along(own.rect.size()));
            let extent = match length > 0.0 {
                true => length + margin,
                false => 0.0,
            };
            let side = usize::from(align == DockAlign::End);
            if ends.with_untracked(|ends| ends[side] != extent) {
                ends.update(|ends| ends[side] = extent);
            }
        }),
    };
    built
}

#[component]
fn DockBarFloat(
    dock: State,
    holder: BarHolder,
    edge: DockEdge,
    align: DockAlign,
    ends: Signal<[f32; 2]>,
) -> NodeId {
    if let BarHolder::Dock = holder {
        return view! {
            <DockBarOverlay dock edge align ends />
        };
    }
    let mode = dock.mode;
    let stacked = move || match holder {
        BarHolder::Tab { stacked, .. } => stacked,
        BarHolder::Dock => mode.get() == DockMode::Stacked,
    };
    let bars = dock.bars_at(holder, edge, true, move |placement| match placement {
        DockBarPlacement::Over(held, at) => held == edge && at == align,
        DockBarPlacement::Window { edge: held, .. } => {
            held == edge && align == DockAlign::Center && stacked()
        }
        DockBarPlacement::Beside(..) => false,
    });
    let margin = dock.bar_margin;
    let held = NodeRef::new();
    let built = view! {
        <DockBarLine dock bars edge align direction={across(edge)} margin ends held />
    };
    let Some(list) = held.try_get() else {
        return built;
    };
    let area = measure(built);
    let covered = measure(list);
    keep_clear(
        list,
        create_memo(move || {
            covered
                .get()
                .map(|covered| vec![Rect::from_min_size(Pos2::ZERO, covered.rect.size())])
                .unwrap_or_default()
        }),
    );
    let BarHolder::Tab { tab, .. } = holder else {
        return built;
    };
    let covers = dock.covers;
    let held_tab = StoredCell::new(None::<TabId>);
    let forget = move || {
        if let Some(tab) = held_tab.get() {
            held_tab.set(None);
            if covers.is_alive() {
                covers.update(|covers| {
                    covers.remove(&(tab, edge, align));
                });
            }
        }
    };
    create_effect(move || {
        let cover = tab
            .get()
            .filter(|_| !bars.with(Vec::is_empty))
            .zip(area.get().zip(covered.get()))
            .map(|(tab, (area, covered))| (tab, covered.rect.translate(-area.rect.min.to_vec2())))
            .filter(|(_, rect)| rect.is_positive());
        untrack(|| {
            if cover.map(|(tab, _)| tab) != held_tab.get() {
                forget();
            }
            if let Some((tab, rect)) = cover {
                held_tab.set(Some(tab));
                if covers.with(|covers| covers.get(&(tab, edge, align)) != Some(&rect)) {
                    covers.update(|covers| {
                        covers.insert((tab, edge, align), rect);
                    });
                }
            }
        });
    });
    on_cleanup(forget);
    built
}

#[component]
fn DockBarOverlay(dock: State, edge: DockEdge, align: DockAlign, ends: Signal<[f32; 2]>) -> NodeId {
    let mode = dock.mode;
    let bars = dock.bars_at(
        BarHolder::Dock,
        edge,
        true,
        move |placement| match placement {
            DockBarPlacement::Over(held, at) => held == edge && at == align,
            DockBarPlacement::Window { edge: held, .. } => {
                held == edge && align == DockAlign::Center && mode.get() == DockMode::Stacked
            }
            DockBarPlacement::Beside(..) => false,
        },
    );
    let margin = dock.bar_margin;
    let line = view! {
        <List direction={across(edge)} spacing={margin}>
            <ForEach keys={bars}>|bar: TabId| {
                let dock = dock;
                view! {
                    <Portal
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        node={Some(dock.bar_panel(bar))}
                    />
                }
            }</ForEach>
        </List>
    };
    let covered = measure(line);
    keep_clear(
        line,
        create_memo(move || {
            covered
                .get()
                .map(|covered| vec![Rect::from_min_size(Pos2::ZERO, covered.rect.size())])
                .unwrap_or_default()
        }),
    );
    let area = dock.rect;
    let along = move |size: Vec2| match edge.direction() {
        Direction::Horizontal => size.x,
        Direction::Vertical => size.y,
    };
    let size = create_memo(move || covered.get().map_or(Vec2::ZERO, |own| own.rect.size()));
    if align != DockAlign::Center {
        create_effect(move || {
            let length = along(size.get());
            let extent = match bars.with(Vec::is_empty) || length <= 0.0 {
                true => 0.0,
                false => length + margin,
            };
            let side = usize::from(align == DockAlign::End);
            if ends.with_untracked(|ends| ends[side] != extent) {
                ends.update(|ends| ends[side] = extent);
            }
        });
    }
    let at = create_memo(move || {
        let rect = area.get();
        let own = size.get();
        let length = along(own);
        let room = along(rect.size()) - margin * 2.0;
        let [start, end] = ends.get();
        let offset = margin
            + match align {
                DockAlign::Start => 0.0,
                DockAlign::End => room - length,
                DockAlign::Center => {
                    let centred = (room - length) / 2.0;
                    centred.min(room - end - length).max(start)
                }
            };
        let pos = match edge {
            DockEdge::Top => pos2(offset, margin),
            DockEdge::Bottom => pos2(offset, rect.height() - margin - own.y),
            DockEdge::Left => pos2(margin, offset),
            DockEdge::Right => pos2(rect.width() - margin - own.x, offset),
        };
        rect.min + pos.to_vec2()
    });
    let anchor = create_memo(move || OverlayAnchor::Point(at.get()));
    let open = create_memo(move || !bars.with(Vec::is_empty) && area.get().is_positive());
    let level = create_memo(move || dock.bar_level());
    let layer = NodeRef::new();
    let view = view! {
        <Overlay
            @node_ref=&layer
            anchor={anchor}
            placement={Placement::Over(0)}
            policy=LayerPolicy::Pass
            level={level}
            traps_focus=false
            open={open}
        >
            <Frame align=Align2::START>
                <Portal node={Some(line)} />
            </Frame>
        </Overlay>
    };
    create_effect(move || {
        if open.get() {
            untrack(|| dock.raise_window(layer));
        }
    });
    view
}

#[component]
pub(super) fn DockBarWindows(dock: State) -> NodeId {
    let state = dock.state;
    let away = dock.bars_away;
    let keys = create_memo(move || {
        if away.get() {
            return Vec::new();
        }
        let windows = state.with(|state| state.bar_windows(&|bar| dock.window_key(bar)));
        state.with(|state| {
            windows
                .into_iter()
                .filter(|(_, bars)| state.followed_bar(bars).is_some())
                .map(|(key, _)| key)
                .collect::<Vec<_>>()
        })
    });
    view! {
        <List spacing=0.0>
            <ForEach keys={keys}>|key: String| {
                let dock = dock;
                view! {
                    <DockBarWindow dock key />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn DockBarWindow(dock: State, key: String) -> NodeId {
    let state = dock.state;
    let shared = key.clone();
    let bars = create_memo(move || {
        let windows = state.with(|state| state.bar_windows(&|bar| dock.window_key(bar)));
        windows
            .into_iter()
            .find(|(held, _)| *held == shared)
            .map(|(_, bars)| bars)
            .unwrap_or_default()
    });
    let followed =
        create_memo(move || bars.with(|bars| state.with(|state| state.followed_bar(bars))));
    let sizes = create_signal(HashMap::<TabId, Vec2>::default());
    let size = create_signal(
        followed
            .get_untracked()
            .and_then(|bar| dock.bar_panel_rect(bar))
            .map(|rect| rect.size())
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(WINDOW_SIZE),
    );
    create_effect(move || {
        let Some(measured) = followed
            .get()
            .and_then(|bar| sizes.with(|sizes| sizes.get(&bar).copied()))
        else {
            return;
        };
        if measured.x > 0.0 && measured.y > 0.0 && size.get_untracked() != measured {
            size.set(measured);
        }
    });
    let origin = create_memo(move || {
        let placement = followed
            .get()
            .and_then(|bar| state.with(|state| state.bar_placement(bar)));
        let origin = match placement {
            Some(DockBarPlacement::Window { origin, .. }) => origin,
            _ => pos2(FLOAT_INSET.x, FLOAT_INSET.y),
        };
        dock.clamped_origin(
            Rect::from_min_size(origin, size.get()),
            dock.rect.get().size(),
        )
    });
    let at = create_memo(move || dock.rect.get().min + origin.get().to_vec2());
    let placed = key.clone();
    let placement = create_memo(move || {
        dock.bar_placements
            .with(|placements| placements.get(&placed).cloned())
            .unwrap_or_else(|| WindowPlacement::Over {
                window: dock.home(),
                at: at.get(),
            })
    });
    let overlay = NodeRef::new();
    let windows = dock.bar_windows;
    let forgotten = key.clone();
    on_cleanup(move || {
        windows.try_with(|windows| windows.borrow_mut().remove(&forgotten));
    });
    let reported = key.clone();
    let view = view! {
        <Window
            @node_ref=&overlay
            id={None}
            size={size}
            at={at}
            level={create_memo(move || dock.bar_level())}
            placement={placement}
            on_report={move |report: WindowReport| match report {
                WindowReport::Placed { placement, .. } => dock.bar_placements.update(|placements| {
                    placements.insert(reported.clone(), WindowPlacement::Host(placement));
                }),
                WindowReport::Closed(_) => {
                    let Some(bar) = followed.get_untracked() else {
                        return;
                    };
                    let edge = dock
                        .state
                        .with_untracked(|state| state.bar_placement(bar))
                        .map_or(DockEdge::Top, DockBarPlacement::edge);
                    dock.place_bar(bar, DockBarPlacement::Over(edge, DockAlign::Center));
                }
                _ => {}
            }}
        >|handle: WindowHandle| {
            windows.value().borrow_mut().insert(key.clone(), handle);
            view! {
                <Root clear_color=Color::TRANSPARENT>
                    <Canvas>
                        <ForEach keys={bars}>|bar: TabId| {
                            let dock = dock;
                            view! {
                                <SharedBar dock bar followed sizes />
                            }
                        }</ForEach>
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
fn SharedBar(
    dock: State,
    bar: TabId,
    followed: Memo<Option<TabId>>,
    sizes: Signal<HashMap<TabId, Vec2>>,
) -> CanvasItem {
    let panel = dock.bar_panel(bar);
    let measured = measure(panel);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            sizes.update(|sizes| {
                sizes.insert(bar, measured.size);
            });
        }
    });
    on_cleanup(move || {
        if sizes.is_alive() {
            sizes.update(|sizes| {
                sizes.remove(&bar);
            });
        }
    });
    let shown = create_memo(move || followed.get() == Some(bar));
    let hidden = create_memo(move || !shown.get());
    let room = create_memo(move || match shown.get() {
        true => WINDOW_ROOM,
        false => 0.0,
    });
    view! {
        <CanvasItem x=0.0 y=0.0 width={room} height={room} clip={hidden} pointer={shown}>
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=WINDOW_ROOM height=WINDOW_ROOM clip=false>
                    <Frame align=Align2::START>
                        <Portal node={Some(panel)} />
                    </Frame>
                </CanvasItem>
            </Canvas>
        </CanvasItem>
    }
}

pub(super) fn shared_key(key: Option<String>, bar: TabId) -> String {
    key.unwrap_or_else(|| format!("#{}", bar.value()))
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
                        <ForEach keys={(0..TARGETS.len()).collect::<Vec<usize>>()}>|index: usize| {
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

#[derive(Clone, Copy)]
pub(super) enum BarHolder {
    Tab {
        tab: Memo<Option<TabId>>,
        stacked: bool,
    },
    Dock,
}

pub(super) type BarPanels = RefCell<HashMap<TabId, (NodeId, Scope)>>;

pub(crate) type BarCovers = HashMap<(TabId, DockEdge, DockAlign), Rect>;
