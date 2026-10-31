use stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use beui_components_unstyled::{DropHandle, DropTarget};
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
const WINDOW_ROOM: f32 = 4096.0;
const WINDOW_SIZE: Vec2 = Vec2::new(240.0, 40.0);
const POP_OFFSET: Vec2 = Vec2::new(16.0, 16.0);
const GAP_SECONDS: f32 = 0.2;
const THROW_SPEED: f32 = 150.0;
const SETTLE: Spring = Spring {
    stiffness: 420.0,
    damping: 32.0,
};
const LEFT: TabId = TabId::new(u64::MAX);
const ARRIVING: TabId = TabId::new(u64::MAX - 1);

pub struct DockBarBandHandle {
    pub edge: DockEdge,
    pub body: NodeId,
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
            surface: NodeRef::new(),
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
            BarBacking::Local(dock) => dock
                .lifting
                .looks(bar)
                .or_else(|| dock.state.with(|state| state.bar_placement(bar))),
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
        if let Some(rect) = rect {
            dock.lift_bar(bar, press.pos, rect);
        }
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
                surface: NodeRef::new(),
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
        match self.fullscreen_over_bars() || self.covered.get() {
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
                self.lift_bar(bar, rect.min + moved.at.to_vec2(), rect);
                self.lifting.lift.update(|lift| {
                    if let Some(lift) = lift {
                        lift.held = true;
                    }
                });
                self.lifting.flight.value().borrow_mut().grow = 0.0;
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

    fn band_slot(&self, bar: TabId, placement: DockBarPlacement, pos: Pos2) -> usize {
        let edge = placement.edge();
        let at = along_of(edge, pos);
        let own = self.layer_of(bar);
        self.stacked_on(bar, placement)
            .iter()
            .filter(|(other, rect)| {
                self.layer_of(*other) == own
                    && (along_start(edge, *rect) + along_end(edge, *rect)) / 2.0 < at
            })
            .count()
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
    ) -> Option<(DockBarPlacement, usize)> {
        let current = self
            .state
            .with_untracked(|state| state.bar_placement(bar))?;
        if point.window != self.home() {
            return None;
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
                return Some((placement, self.band_slot(bar, placement, point.pos)));
            }
            let align = along_third(body, edge, point.pos);
            let placement = DockBarPlacement::Over(edge, align);
            let stacked = self.stacked_on(bar, placement);
            let over_far = stacked
                .iter()
                .map(|(_, rect)| reach_far(pane, *rect, edge))
                .fold(0.0_f32, f32::max);
            if distance >= (beside + band * 2.0).max(over_far + band) {
                return None;
            }
            let own = self.layer_of(bar);
            let slot = stacked
                .iter()
                .filter(|(other, rect)| {
                    self.layer_of(*other) == own && centre_reach(pane, *rect, edge) < distance
                })
                .count();
            return Some((placement, slot));
        }
        let dock = self.rect.get_untracked();
        let origin = point.pos - dock.min.to_vec2() - grab;
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        Some((
            DockBarPlacement::Window {
                edge: current.edge(),
                origin,
            },
            0,
        ))
    }

    pub(super) fn drop_bar(&self, bar: TabId, point: DragPoint) {
        let lifted = self.lifting.lifts(bar);
        let held = self
            .lifting
            .lift
            .get_untracked()
            .is_some_and(|lift| lift.held);
        let from = held.then(|| self.bar_panel_rect(bar)).flatten();
        let point = match lifted {
            true => self.thrown(bar, point),
            false => point,
        };
        let target = self.resolve_bar(point, bar);
        let Some((placement, slot)) = target.or_else(|| {
            let state = self.state.get_untracked();
            Some((state.bar_placement(bar)?, state.bar_slot(bar)))
        }) else {
            self.end_lift();
            return;
        };
        if placement.is_window() {
            self.place_bar(bar, placement);
            self.end_lift();
            return;
        }
        let dock = *self;
        self.edit(|state| {
            state.place_bar_at(bar, placement, slot);
            dock.share_placement(state, bar);
        });
        if lifted {
            self.settle_lift(placement, slot, from);
        }
    }

    fn bar_outside(&self, bar: TabId, _: DragPoint) {
        if !self.opens.get_untracked() {
            return;
        }
        self.end_lift();
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
        let away = self.bars_away;
        let dock = *self;
        let lifting = self.lifting;
        create_memo(move || {
            let tab = match holder {
                BarHolder::Tab { tab, .. } => match tab.get().filter(|_| !away.get()) {
                    Some(tab) => Some(tab),
                    None => return Vec::new(),
                },
                BarHolder::Dock => None,
            };
            let pick = |state: &DockState| -> Vec<TabId> {
                let held = match tab {
                    Some(tab) => state.pane_bars(tab),
                    None => state.bars_of(None),
                };
                held.into_iter()
                    .filter(|bar| !state.is_bar_hidden(*bar))
                    .filter(|bar| state.bar_placement(*bar).is_some_and(&wanted))
                    .collect()
            };
            let mut bars = dock.shown(&pick);
            let slot = |bars: &[TabId], bar: TabId, there: &DockState| {
                let was = pick(there);
                let at = was.iter().position(|held| *held == bar)?;
                let before = was[at + 1..]
                    .iter()
                    .find_map(|next| bars.iter().position(|held| held == next));
                Some(before.unwrap_or(bars.len()))
            };
            let held = lifting.lift.with(|lift| {
                let lift = lift.filter(|lift| lift.held)?;
                let (placement, slot) = lift.target?;
                let home = dock.state.with(|state| {
                    state.bar_placement(lift.bar) == Some(placement)
                        && state.bar_slot(lift.bar) == slot
                });
                (!home).then_some(lift.bar)
            });
            if let Some(bar) = held {
                let at = lifting
                    .preview
                    .with(|preview| slot(&bars, bar, preview.as_ref()?));
                if let Some(at) = at {
                    bars.insert(at, ARRIVING);
                }
            }
            let left = lifting.left.with(|left| {
                left.as_ref()
                    .and_then(|(bar, left)| slot(&bars, *bar, left))
            });
            if let Some(at) = left {
                bars.insert(at, LEFT);
            }
            if stacked && matches!(edge, DockEdge::Bottom | DockEdge::Right) {
                bars.reverse();
            }
            bars
        })
    }

    pub(super) fn shown<R>(&self, read: impl FnOnce(&DockState) -> R) -> R {
        let mut read = Some(read);
        let held = self
            .lifting
            .lift
            .with(|lift| lift.is_some_and(|lift| lift.held));
        let previewed = self.lifting.preview.with(|preview| {
            let preview = preview.as_ref().filter(|_| !held)?;
            Some(read.take()?(preview))
        });
        match previewed {
            Some(value) => value,
            None => self
                .state
                .with(|state| read.take().expect("read once")(state)),
        }
    }

    pub(super) fn lift_bar(&self, bar: TabId, pointer: Pos2, rect: Rect) {
        let state = self.state.get_untracked();
        let Some(placement) = state.bar_placement(bar) else {
            return;
        };
        let target = (!placement.is_window()).then(|| (placement, state.bar_slot(bar)));
        let lifting = self.lifting;
        *lifting.flight.value().borrow_mut() = Flight {
            pos: rect.min,
            speed: Vec2::ZERO,
            pointer,
            grab: pointer - rect.min,
            velocity: Velocity::new(now(), pointer),
            grow: 1.0,
            shrink: 0.0,
        };
        lifting.at.set(rect.min);
        lifting.size.set(rect.size());
        lifting.gap.set(None);
        lifting.gone.set(None);
        lifting.growth.set((1.0, 0.0));
        lifting.lift.set(Some(Lift {
            bar,
            edge: placement.edge(),
            from: Some(placement)
                .filter(|placement| state.is_dock_bar(bar) && !placement.is_beside()),
            held: false,
            target,
            settling: false,
        }));
    }

    pub(super) fn move_lift(&self, point: DragPoint) {
        let Some(lift) = self.lifting.lift.get_untracked() else {
            return;
        };
        if lift.settling {
            return;
        }
        {
            let flight = self.lifting.flight.value();
            let mut flight = flight.borrow_mut();
            flight.pointer = point.pos;
            flight.velocity.sample(now(), point.pos);
        }
        let target = self
            .resolve_bar(point, lift.bar)
            .filter(|(placement, _)| !placement.is_window());
        self.retarget(target);
        self.animate_lift();
    }

    fn retarget(&self, target: Option<(DockBarPlacement, usize)>) {
        let lifting = self.lifting;
        let Some(lift) = lifting.lift.get_untracked() else {
            return;
        };
        if lift.target == target {
            return;
        }
        {
            let flight = lifting.flight.value();
            let mut flight = flight.borrow_mut();
            let gone = lifting.gone.get_untracked();
            match (gone, lift.target) {
                (Some(gone), _) if Some(gone.place) == target => {
                    flight.grow = flight.shrink;
                    flight.shrink = 0.0;
                    lifting.gone.set(None);
                }
                (_, Some(place)) => {
                    flight.shrink = flight.grow;
                    flight.grow = 0.0;
                    lifting.gone.set(Some(Gone {
                        place,
                        size: lifting.size.get_untracked(),
                    }));
                }
                (_, None) => flight.grow = 0.0,
            }
            lifting
                .growth
                .set((ease(flight.grow), 1.0 - ease(1.0 - flight.shrink)));
        }
        lifting.gap.set(None);
        lifting.lift.set(Some(Lift { target, ..lift }));
    }

    fn settle_lift(&self, placement: DockBarPlacement, slot: usize, from: Option<Rect>) {
        let lifting = self.lifting;
        {
            let flight = lifting.flight.value();
            let mut flight = flight.borrow_mut();
            flight.speed = flight.velocity.get();
            if let Some(from) = from {
                flight.pos = from.min;
                lifting.at.set(from.min);
                lifting.size.set(from.size());
            }
        }
        self.retarget(Some((placement, slot)));
        lifting.lift.update(|lift| {
            if let Some(lift) = lift {
                lift.settling = true;
                lift.held = false;
            }
        });
        self.animate_lift();
    }

    pub(super) fn drop_lift(&self) {
        if self
            .lifting
            .lift
            .get_untracked()
            .is_some_and(|lift| !lift.settling)
        {
            self.end_lift();
        }
    }

    fn end_lift(&self) {
        let lifting = self.lifting;
        if !lifting.lift.is_alive() {
            return;
        }
        if lifting.lift.get_untracked().is_some() {
            lifting.lift.set(None);
        }
        lifting.gone.set(None);
        lifting.gap.set(None);
        if let Some(motion) = lifting.motion.get() {
            motion.stop();
        }
    }

    fn animate_lift(&self) {
        if let Some(motion) = self.lifting.motion.get() {
            motion.start();
        }
    }

    fn thrown(&self, bar: TabId, point: DragPoint) -> DragPoint {
        let velocity = {
            let flight = self.lifting.flight.value();
            let mut flight = flight.borrow_mut();
            flight.velocity.sample(now(), point.pos);
            flight.velocity.get()
        };
        let Some((_, pane)) = self.bar_areas(bar) else {
            return point;
        };
        if point.window != self.home() || !pane.contains(point.pos) || !motion().animates() {
            return point;
        }
        let reach = |speed: f32| match speed.abs() < THROW_SPEED {
            true => 0.0,
            false => Fling::new(speed).map_or(0.0, |fling| fling.distance()),
        };
        let to = point.pos + vec2(reach(velocity.x), reach(velocity.y));
        let inside = pane.shrink(1.0);
        DragPoint {
            pos: pos2(
                to.x.clamp(inside.left(), inside.right()),
                to.y.clamp(inside.top(), inside.bottom()),
            ),
            ..point
        }
    }

    fn lift_goal(&self, lift: Lift, flight: &Flight) -> Option<Pos2> {
        let gap = self.lifting.gap.get_untracked();
        if lift.settling {
            return gap.map(|gap| gap.min);
        }
        let size = self.lifting.size.get_untracked();
        let grab = vec2(
            flight.grab.x.clamp(0.0, size.x.max(0.0)),
            flight.grab.y.clamp(0.0, size.y.max(0.0)),
        );
        let free = flight.pointer - grab;
        Some(match (lift.target, gap) {
            (Some((DockBarPlacement::Beside(edge, _), _)), Some(gap)) => match edge.direction() {
                Direction::Horizontal => pos2(free.x, gap.min.y),
                Direction::Vertical => pos2(gap.min.x, free.y),
            },
            _ => free,
        })
    }

    fn step_lift(&self, frame: AnimationFrame) -> bool {
        let lifting = self.lifting;
        let Some(lift) = lifting.lift.get_untracked() else {
            return false;
        };
        let seconds = frame.delta.as_secs_f32();
        if seconds <= 0.0 {
            return false;
        }
        let flight = lifting.flight.value();
        let mut flight = flight.borrow_mut();
        flight.grow = (flight.grow + seconds / GAP_SECONDS).min(1.0);
        flight.shrink = (flight.shrink - seconds / GAP_SECONDS).max(0.0);
        if flight.shrink <= 0.0 && lifting.gone.get_untracked().is_some() {
            lifting.gone.set(None);
        }
        let growth = (ease(flight.grow), 1.0 - ease(1.0 - flight.shrink));
        if lifting.growth.get_untracked() != growth {
            lifting.growth.set(growth);
        }
        let goal = self.lift_goal(lift, &flight);
        if let Some(goal) = goal {
            match lift.settling {
                true => {
                    let mut off = flight.pos - goal;
                    let mut speed = flight.speed;
                    SETTLE.advance(&mut off.x, &mut speed.x, seconds);
                    SETTLE.advance(&mut off.y, &mut speed.y, seconds);
                    flight.pos = goal + off;
                    flight.speed = speed;
                }
                false => {
                    flight.pos = goal;
                    flight.speed = Vec2::ZERO;
                }
            }
            if lifting.at.get_untracked() != flight.pos {
                lifting.at.set(flight.pos);
            }
        }
        let resting = goal == Some(flight.pos) && flight.speed == Vec2::ZERO;
        let growing = flight.grow < 1.0 || flight.shrink > 0.0;
        if !lift.settling {
            return !resting || growing;
        }
        let placed = lifting.gap.get_untracked().is_some();
        !(resting && placed && !growing)
    }

    fn finish_lift(&self) {
        let lifting = self.lifting;
        if !lifting.lift.is_alive() {
            return;
        }
        let Some(lift) = lifting.lift.get_untracked() else {
            return;
        };
        if lift.settling {
            self.end_lift();
            return;
        }
        let flight = lifting.flight.value();
        let mut flight = flight.borrow_mut();
        flight.grow = 1.0;
        flight.shrink = 0.0;
        flight.speed = Vec2::ZERO;
        lifting.gone.set(None);
        lifting.growth.set((1.0, 0.0));
        if let Some(goal) = self.lift_goal(lift, &flight) {
            flight.pos = goal;
            lifting.at.set(goal);
        }
    }

    pub(super) fn start_lifting(&self) {
        let dock = *self;
        let motion = create_animation(
            move |frame| dock.step_lift(frame),
            move || dock.finish_lift(),
        );
        self.lifting.motion.set(Some(motion));
    }
}

fn ease(progress: f32) -> f32 {
    tween(progress, 1.0, Easing::OutCubic)
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Lift {
    bar: TabId,
    edge: DockEdge,
    from: Option<DockBarPlacement>,
    held: bool,
    target: Option<(DockBarPlacement, usize)>,
    settling: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Gone {
    place: (DockBarPlacement, usize),
    size: Vec2,
}

#[derive(Default)]
struct Flight {
    pos: Pos2,
    speed: Vec2,
    pointer: Pos2,
    grab: Vec2,
    velocity: Velocity,
    grow: f32,
    shrink: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Lifting {
    lift: Signal<Option<Lift>>,
    gone: Signal<Option<Gone>>,
    at: Signal<Pos2>,
    size: Signal<Vec2>,
    gap: Signal<Option<Rect>>,
    growth: Signal<(f32, f32)>,
    flight: Stored<RefCell<Flight>>,
    motion: StoredCell<Option<Animation>>,
    preview: Memo<Option<DockState>>,
    left: Memo<Option<(TabId, DockState)>>,
}

impl Lifting {
    pub(super) fn new(state: Binding<DockState>) -> Self {
        let lift = create_signal(None::<Lift>);
        let gone = create_signal(None::<Gone>);
        let preview = create_memo(move || {
            let lift = lift.get()?;
            let mut state = state.get();
            match lift.target {
                Some((placement, slot)) => state.place_bar_at(lift.bar, placement, slot),
                None => state.set_bar_hidden(lift.bar, true),
            }
            Some(state)
        });
        let left = create_memo(move || {
            let lift = lift.get()?;
            let gone = gone.get()?;
            let mut state = state.get();
            let (placement, slot) = gone.place;
            state.place_bar_at(lift.bar, placement, slot);
            Some((lift.bar, state))
        });
        Self {
            lift,
            gone,
            at: create_signal(Pos2::ZERO),
            size: create_signal(Vec2::ZERO),
            gap: create_signal(None),
            growth: create_signal((1.0, 0.0)),
            flight: store(RefCell::new(Flight::default())),
            motion: StoredCell::new(None),
            preview,
            left,
        }
    }

    fn lifts(&self, bar: TabId) -> bool {
        self.lift
            .get_untracked()
            .is_some_and(|lift| lift.bar == bar)
    }

    fn looks(&self, bar: TabId) -> Option<DockBarPlacement> {
        let lift = self
            .lift
            .get()
            .filter(|lift| lift.bar == bar && !lift.held)?;
        Some(match lift.target {
            Some((placement, _)) => placement,
            None => DockBarPlacement::Window {
                edge: lift.edge,
                origin: Pos2::ZERO,
            },
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

fn centre_reach(pane: Rect, rect: Rect, edge: DockEdge) -> f32 {
    (reach(pane, rect, edge) + reach_far(pane, rect, edge)) / 2.0
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
    let band = view! {
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
    };
    if let BarHolder::Tab { .. } = holder {
        return band;
    }
    view! {
        <DropTarget
            on_over={move |point: Option<(DockDragged, DragPoint)>| dock.drag_over(point.map(|(_, point)| point))}
            on_drop={move |(dragged, point): (DockDragged, DragPoint)| dock.drop_at(dragged, point)}
        >|_: DropHandle| {
            band
        }</DropTarget>
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
            placement
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
    placement: DockBarPlacement,
    direction: Direction,
    margin: f32,
    ends: Signal<[f32; 2]>,
    held: NodeRef,
) -> NodeId {
    let (edge, align) = (placement.edge(), placement.align());
    let padding = create_signal(Sides::all(margin));
    let built = view! {
        <Frame align={float_align(edge, align)} padding={padding}>
            <List @node_ref=&held direction={direction} spacing={margin}>
                <ForEach keys={bars}>|bar: TabId| {
                    let dock = dock;
                    view! {
                        <DockBarSlot dock bar placement @sizing={ItemSize::Intrinsic.shrink(1.0)} />
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
        <DockBarLine
            dock
            bars
            placement={DockBarPlacement::Over(edge, align)}
            direction={across(edge)}
            margin
            ends
            held
        />
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
                let placement = DockBarPlacement::Over(edge, align);
                view! {
                    <DockBarSlot dock bar placement @sizing={ItemSize::Intrinsic.shrink(1.0)} />
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
    let lifting = dock.lifting;
    let home = DockBarPlacement::Over(edge, align);
    let lifted_here = create_memo(move || {
        lifting
            .lift
            .with(|lift| lift.is_some_and(|lift| lift.from == Some(home)))
    });
    let open = create_memo(move || {
        (!bars.with(Vec::is_empty) || lifted_here.get()) && area.get().is_positive()
    });
    let floats = dock.floats;
    let float = move |shown: Option<Rect>| {
        if !floats.is_alive()
            || floats.with_untracked(|floats| floats.get(&(edge, align)).copied()) == shown
        {
            return;
        }
        floats.update(|floats| match shown {
            Some(rect) => {
                floats.insert((edge, align), rect);
            }
            None => {
                floats.remove(&(edge, align));
            }
        });
    };
    create_effect(move || {
        let shown = open
            .get()
            .then(|| Rect::from_min_size(at.get(), size.get()))
            .filter(Rect::is_positive);
        untrack(|| float(shown));
    });
    on_cleanup(move || float(None));
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
    let away = dock.bars_away;
    let keys = create_memo(move || {
        if away.get() {
            return Vec::new();
        }
        dock.shown(|state| {
            state
                .bar_windows(&|bar| dock.window_key(bar))
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
        let windows = dock.shown(|state| state.bar_windows(&|bar| dock.window_key(bar)));
        windows
            .into_iter()
            .find(|(held, _)| *held == shared)
            .map(|(_, bars)| bars)
            .unwrap_or_default()
    });
    let followed =
        create_memo(move || bars.with(|bars| dock.shown(|state| state.followed_bar(bars))));
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
    let parent = dock.watch_home();
    let placed = key.clone();
    let placement = create_memo(move || {
        dock.bar_placements
            .with(|placements| placements.get(&placed).cloned())
            .unwrap_or_else(|| WindowPlacement::Over {
                window: parent.get(),
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

#[component]
fn DockBarSlot(dock: State, bar: TabId, placement: DockBarPlacement) -> NodeId {
    let lifting = dock.lifting;
    let left = bar == LEFT;
    let ghost = bar == ARRIVING;
    let arriving = create_memo(move || {
        !left
            && lifting.lift.with(|lift| {
                lift.is_some_and(|lift| {
                    (lift.bar == bar && !lift.held || ghost)
                        && lift
                            .target
                            .is_some_and(|(target, _)| target.same_place(placement))
                })
            })
    });
    let lifted = create_memo(move || {
        left || ghost
            || lifting
                .lift
                .with(|lift| lift.is_some_and(|lift| lift.bar == bar && !lift.held))
    });
    let size = create_memo(move || {
        if arriving.get() {
            return lifting.size.get() * lifting.growth.get().0;
        }
        if left {
            let shrink = lifting.growth.get().1;
            return lifting
                .gone
                .get()
                .map_or(Vec2::ZERO, |gone| gone.size * shrink);
        }
        Vec2::ZERO
    });
    let oriented = create_memo(move || {
        let size = size.get();
        let (long, short) = (size.x.max(size.y), size.x.min(size.y));
        match placement.edge().direction() {
            Direction::Horizontal => vec2(long, short),
            Direction::Vertical => vec2(short, long),
        }
    });
    let width = create_memo(move || oriented.get().x);
    let height = create_memo(move || oriented.get().y);
    let gap = view! {
        <Frame width=width height=height />
    };
    let measured = measure(gap);
    create_effect(move || {
        if !arriving.get() {
            return;
        }
        let Some(rect) = measured.get().map(|measured| measured.rect) else {
            return;
        };
        untrack(|| {
            if lifting.gap.get() != Some(rect) {
                lifting.gap.set(Some(rect));
                dock.animate_lift();
            }
        });
    });
    let node = create_memo(move || match lifted.get() {
        true => Some(gap),
        false => Some(dock.bar_panel(bar)),
    });
    view! {
        <Portal node={node} />
    }
}

#[component]
pub(super) fn DockBarLifted(dock: State) -> NodeId {
    let lifting = dock.lifting;
    let lifted = create_memo(move || {
        lifting
            .lift
            .with(|lift| lift.filter(|lift| !lift.held).map(|lift| lift.bar))
    });
    let node = create_memo(move || lifted.get().map(|bar| dock.bar_panel(bar)));
    let held = view! {
        <Portal node={node} />
    };
    let measured = measure(held);
    create_effect(move || {
        let Some(size) = measured.get().map(|measured| measured.rect.size()) else {
            return;
        };
        if size.x > 0.0 && size.y > 0.0 && lifted.get().is_some() {
            untrack(|| {
                if lifting.size.get() != size {
                    lifting.size.set(size);
                }
            });
        }
    });
    let anchor = create_memo(move || OverlayAnchor::Point(lifting.at.get()));
    let open = create_memo(move || lifted.get().is_some());

    let layer = NodeRef::new();
    let view = view! {
        <Overlay
            @node_ref=&layer
            anchor={anchor}
            placement=Placement::BelowStart
            policy=LayerPolicy::Inert
            level={create_memo(move || dock.bar_level())}
            traps_focus=false
            open={open}
        >
            <Frame align=Align2::START>{held}</Frame>
        </Overlay>
    };
    create_effect(move || {
        if open.get() {
            untrack(|| dock.raise_window(layer));
        }
    });
    view
}

pub(super) fn shared_key(key: Option<String>, bar: TabId) -> String {
    key.unwrap_or_else(|| format!("#{}", bar.value()))
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

pub(crate) type BarFloats = HashMap<(DockEdge, DockAlign), Rect>;
