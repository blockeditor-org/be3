//! Tool bars (`DockBar`): bands, floating runs and popped-out windows, and carrying a bar the user
//! drags. A dragged bar is lifted into its own `LayerPolicy::Inert` layer at the bars' level and a
//! preview `DockState` shows where it would land; every point is read in the dock's own window, so a
//! press begun in another layer still steers. Each run of floating dock bars is its own layer at
//! `LayerLevel::Raised`: floating dock windows pass under it, menus and modals over it.

use be_lib_stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use beui_components_unstyled::{DragSource, DropHandle, DropTarget, drag_board};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::components::root::Root;
use beui_view::drag::DragPoint;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::window::{WindowPlacement, WindowReport};
use beui_view::types::{
    AboveEmbeds, Align, Align2, Color, Direction, LayerLevel, LayerPolicy, Placement, PointerPress,
};
use beui_view::types::{Pos2, Rect, Sides, Vec2, pos2, vec2};
use beui_view::window::{Window, WindowHandle};

use beui_dock::{
    BarAutoHide, DockBarControl, DockBarDriver, DockBarMove, DockDragStage, DockRequest, bar_menu,
};

use super::reveal::DockBarHidden;
use super::{
    DockAlign, DockBarPlacement, DockDragged, DockEdge, DockMode, DockState, FLOAT_INSET, LeafId,
    State, TabId,
};

const BAND_SHARE: f32 = 0.12;
const BAND_MIN: f32 = 24.0;
const BAND_MAX: f32 = 56.0;
const WINDOW_ROOM: f32 = 4096.0;
const WINDOW_SIZE: Vec2 = Vec2::new(240.0, 40.0);
const POP_OFFSET: Vec2 = Vec2::new(16.0, 16.0);
/// How long a gap takes to open under a dragged bar while the layout previews where it would land.
const GAP_SECONDS: f32 = 0.2;
/// A bar let go at this pointer speed or more outside every band is thrown: it floats over the
/// edge its path runs into and never docks.
const THROW_SPEED: f32 = 300.0;
/// How near an edge, in bands (the band there, else the bar's thickness), the pointer must come
/// for a bar to dock in that edge's band.
const DOCK_REACH: f32 = 0.5;
/// How far out, in bands, a bar docked in a band (or about to be) holds on to it.
const DOCK_HOLD: f32 = 1.5;
/// A bar grabbed while floating over an edge docks there only once the pointer has come this
/// share of the way from where it grabbed towards the edge, so a grab alone keeps it floating.
const FLOAT_HOLD: f32 = 0.5;
const SETTLE: Spring = Spring {
    stiffness: 420.0,
    damping: 32.0,
};
const LEFT: TabId = TabId::new(u64::MAX);

pub struct DockBarBandHandle {
    pub edge: DockEdge,
    pub body: NodeId,
}

impl DockBarDriver for State {
    fn placement(&self, bar: TabId) -> Option<DockBarPlacement> {
        if !self.panels.is_alive() {
            return None;
        }
        self.lifting
            .looks(bar)
            .or_else(|| self.state.with(|state| state.bar_placement(bar)))
    }

    fn is_hidden(&self, bar: TabId) -> bool {
        self.panels.is_alive() && self.state.with(|state| state.is_bar_hidden(bar))
    }

    fn set_hidden(&self, bar: TabId, hidden: bool) {
        if self.panels.is_alive() {
            self.hide_bar(bar, hidden);
        }
    }

    fn moving(&self, bar: TabId) -> bool {
        self.panels.is_alive()
            && self.drag.with(|drag| {
                drag.as_ref()
                    .is_some_and(|drag| drag.dragged == DockDragged::Bar(bar))
            })
    }

    fn place(&self, bar: TabId, placement: DockBarPlacement) {
        if self.panels.is_alive() {
            self.place_bar(bar, placement);
        }
    }

    fn pop_out(&self, bar: TabId) {
        if self.panels.is_alive() {
            self.pop_out_bar(bar);
        }
    }

    fn hold_band(&self, bar: TabId, held: bool) {
        if self.panels.is_alive() {
            self.reveal.hold(bar, held);
        }
    }

    fn begin_move(&self, bar: TabId, press: PointerPress) {
        let dock = *self;
        if !dock.panels.is_alive() || dock.mode.get_untracked() != DockMode::Tiled {
            return;
        }
        if untrack(|| DockBarDriver::moving(self, bar)) {
            drag_board().track(DragPoint::of(press));
            return;
        }
        if dock.moves_bar_window(bar) {
            return;
        }
        let grab = dock
            .bar_panel_rect(bar)
            .map_or(Vec2::ZERO, |rect| press.pos - rect.min);
        let rect = dock.bar_home_rect(bar);
        let size = rect.map_or(WINDOW_SIZE, |rect| rect.size());
        dock.bar_grab.set((grab, size));
        dock.begin_drag(DockDragged::Bar(bar));
        if let Some(rect) = rect {
            dock.lift_bar(bar, rect.min + grab, rect);
        }
        drag_board().begin(
            DragSource {
                outside: Rc::new(move |point: DragPoint| dock.bar_outside(bar, point)),
                ..DragSource::new(Rc::new(DockDragged::Bar(bar)))
            },
            DragPoint::of(press),
        );
    }
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
            let auto_hide = BarAutoHide {
                offered: create_memo(move || dock.offers_auto_hide(bar)),
                on: create_memo(move || {
                    let edge = placement.get().map(DockBarPlacement::edge);
                    state.with(|state| edge.is_some_and(|edge| state.is_edge_auto_hidden(edge)))
                }),
                set: Callback::new(move |hidden: bool| dock.auto_hide_band(bar, hidden)),
            };
            provide_context(DockBarControl::local(
                bar,
                Rc::new(dock),
                bar_menu(placement, place, hide, Some(auto_hide)),
            ));
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

    fn auto_hide_band(&self, bar: TabId, hidden: bool) {
        self.edit(|state| {
            if let Some(placement) = state.bar_placement(bar) {
                state.set_edge_auto_hidden(placement.edge(), hidden);
            }
        });
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

    fn moves_bar_window(&self, bar: TabId) -> bool {
        let windowed = self
            .state
            .with_untracked(|state| state.bar_placement(bar))
            .is_some_and(DockBarPlacement::is_window);
        if !windowed || self.bar_window_in_dock(bar) {
            return false;
        }
        let key = self.window_key(bar);
        let handle = self.bar_windows.value().borrow().get(&key).copied();
        handle.is_some_and(WindowHandle::begin_move)
    }

    fn carry_bar(&self, bar: TabId, moved: DockBarMove) {
        if self.lifting.window_moved.get() == Some(bar) {
            if moved.stage != DockDragStage::Move {
                self.lifting.window_moved.set(None);
            }
            return;
        }
        let carried = self.drag.with_untracked(|drag| {
            drag.as_ref()
                .is_some_and(|drag| drag.dragged == DockDragged::Bar(bar))
        });
        let rect = match carried {
            true => self
                .lifting
                .carried
                .get()
                .or_else(|| self.bar_home_rect(bar)),
            false => self.bar_home_rect(bar),
        };
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
                if self.moves_bar_window(bar) {
                    self.lifting.window_moved.set(Some(bar));
                    return;
                }
                self.lifting.carried.set(Some(rect));
                self.bar_grab.set((moved.at.to_vec2(), rect.size()));
                self.begin_drag(DockDragged::Bar(bar));
                self.lift_bar(bar, rect.min + moved.at.to_vec2(), rect);
                self.drag_over(point);
            }
            DockDragStage::Drop if carried => {
                self.lifting.carried.set(None);
                if let Some(point) = point {
                    self.drop_bar(bar, point);
                }
                self.end_drag();
            }
            DockDragStage::Cancel if carried => {
                self.lifting.carried.set(None);
                self.end_drag();
            }
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

    fn bar_window_in_dock(&self, bar: TabId) -> bool {
        if !self.opens.get_untracked() {
            return true;
        }
        let key = self.window_key(bar);
        let window = self
            .bar_windows
            .value()
            .borrow()
            .get(&key)
            .map(|handle| handle.id);
        window.is_none_or(|window| window_point(Pos2::ZERO, window, self.home()).is_some())
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
        match state.is_dock_bar(bar) {
            true => self.areas(self.dock_body, self.dock_pane),
            false => self.pane_areas(state.bar_tab(bar)?),
        }
    }

    /// The body and pane rects of the pane showing `tab`.
    fn pane_areas(&self, tab: TabId) -> Option<(Rect, Rect)> {
        let state = self.state.get_untracked();
        let leaf = state.find(tab)?.leaf;
        if !state.is_shown(tab, DockMode::Tiled) {
            return None;
        }
        let (body, pane) = *self.bodies.value().borrow().get(&leaf)?;
        self.areas(body, pane)
    }

    /// Where a bar carried to `pos` would land: on the dock for its own bars, else on whichever
    /// of the panes it may show on (`DockState::bar_panes`) holds the point, its own first.
    fn pane_at(&self, bar: TabId, pos: Pos2) -> Option<(Option<TabId>, Rect, Rect)> {
        let state = self.state.get_untracked();
        if state.is_dock_bar(bar) {
            let (body, pane) = self.bar_areas(bar)?;
            return pane.contains(pos).then_some((None, body, pane));
        }
        let own = state.bar_tab(bar);
        own.into_iter()
            .chain(state.bar_panes(bar).into_iter().filter(|tab| Some(*tab) != own))
            .find_map(|tab| {
                let (body, pane) = self.pane_areas(tab)?;
                pane.contains(pos).then_some((Some(tab), body, pane))
            })
    }

    fn areas(&self, body: NodeRef, pane: NodeRef) -> Option<(Rect, Rect)> {
        let body = measure(body.try_get()?).get_untracked()?.rect;
        let pane = pane
            .try_get()
            .and_then(|pane| measure(pane).get_untracked())
            .map_or(body, |pane| pane.rect);
        Some((body, pane))
    }

    fn stacked_on(
        &self,
        bar: TabId,
        pane: Option<TabId>,
        placement: DockBarPlacement,
    ) -> Vec<(TabId, Rect)> {
        let state = self.state.get_untracked();
        let held = match pane {
            Some(tab) => state.pane_bars(tab),
            None => state.bars_with(bar),
        };
        held.into_iter()
            .filter(|other| *other != bar && state.bar_placement(*other) == Some(placement))
            .filter_map(|other| Some((other, self.bar_panel_rect(other)?)))
            .collect()
    }

    fn band_align(
        &self,
        bar: TabId,
        pane: Option<TabId>,
        edge: DockEdge,
        pos: Pos2,
    ) -> Option<DockAlign> {
        let at = along_of(edge, pos);
        DockAlign::ALL.into_iter().find(|align| {
            let lined = self.stacked_on(bar, pane, DockBarPlacement::Beside(edge, *align));
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
        pane: Option<TabId>,
        placement: DockBarPlacement,
        pos: Pos2,
    ) -> usize {
        let edge = placement.edge();
        let at = along_of(edge, pos);
        self.stacked_on(bar, pane, placement)
            .iter()
            .filter(|(other, rect)| {
                self.same_layer(bar, *other)
                    && (along_start(edge, *rect) + along_end(edge, *rect)) / 2.0 < at
            })
            .count()
    }

    /// Whether two bars belong to one layer: a drop slots a bar only among its own owner's bars,
    /// as `DockState::place_bar_at` counts them.
    fn same_layer(&self, bar: TabId, other: TabId) -> bool {
        self.state
            .with_untracked(|state| state.bar_owner(bar) == state.bar_owner(other))
    }

    pub(super) fn resolve_bar(
        &self,
        point: DragPoint,
        bar: TabId,
    ) -> Option<BarPlace> {
        let current = self
            .state
            .with_untracked(|state| state.bar_placement(bar))?;
        if point.window != self.home() {
            return None;
        }
        let (grab, size) = self.bar_grab.get();
        if let Some((on, body, pane)) = self.pane_at(bar, point.pos) {
            let (edge, distance) = nearest_edge(pane, point.pos);
            let beside = reach(pane, body, edge);
            let band = band(body);
            if distance < self.dock_reach(edge, beside, size) {
                let align = self
                    .band_align(bar, on, edge, point.pos)
                    .unwrap_or_else(|| along_third(body, edge, point.pos));
                let placement = DockBarPlacement::Beside(edge, align);
                return Some(BarPlace {
                    placement,
                    slot: self.band_slot(bar, on, placement, point.pos),
                    pane: on,
                });
            }
            let align = along_third(body, edge, point.pos);
            let placement = DockBarPlacement::Over(edge, align);
            let stacked = self.stacked_on(bar, on, placement);
            let over_far = stacked
                .iter()
                .map(|(_, rect)| reach_far(pane, *rect, edge))
                .fold(0.0_f32, f32::max);
            if distance >= (beside + band * 2.0).max(over_far + band) {
                return None;
            }
            let slot = stacked
                .iter()
                .filter(|(other, rect)| {
                    self.same_layer(bar, *other) && centre_reach(pane, *rect, edge) < distance
                })
                .count();
            return Some(BarPlace {
                placement,
                slot,
                pane: on,
            });
        }
        let dock = self.rect.get_untracked();
        let origin = point.pos - dock.min.to_vec2() - grab;
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        Some(BarPlace {
            placement: DockBarPlacement::Window {
                edge: current.edge(),
                origin,
            },
            slot: 0,
            pane: None,
        })
    }

    /// How near `edge` the pointer docks a carried bar of `size` in its band, `beside` deep: half a
    /// band to come in, and half again past the band to stay, so the band neither snatches a bar
    /// passing near it nor drops one nudged along it. A bar grabbed floating over the edge must
    /// first be pulled towards it.
    fn dock_reach(&self, edge: DockEdge, beside: f32, size: Vec2) -> f32 {
        let depth = beside.max(size.x.min(size.y));
        let lift = self.lifting.lift.get_untracked();
        let docked = lift
            .and_then(|lift| lift.target)
            .is_some_and(|target| target.placement.is_beside() && target.placement.edge() == edge);
        if docked {
            return depth * DOCK_HOLD;
        }
        let reach = depth * DOCK_REACH;
        match lift.and_then(|lift| lift.floated) {
            Some((floated, grabbed)) if floated == edge => reach.min(grabbed * FLOAT_HOLD),
            _ => reach,
        }
    }

    fn at_home(&self, point: DragPoint) -> DragPoint {
        let home = self.home();
        match window_point(point.pos, point.window, home) {
            Some(pos) => DragPoint {
                pos,
                window: home,
                ..point
            },
            None => point,
        }
    }

    pub(super) fn drop_bar(&self, bar: TabId, point: DragPoint) {
        let point = self.at_home(point);
        let lifted = self.lifting.lifts(bar);
        let resolved = self.resolve_bar(point, bar);
        let docks = resolved.is_some_and(|target| target.placement.is_beside());
        let target = lifted
            .then(|| self.thrown(bar, point))
            .flatten()
            .filter(|_| !docks)
            .or(resolved)
            .or_else(|| self.float_near(bar, point));
        let Some(target) = target.or_else(|| {
            let state = self.state.get_untracked();
            Some(BarPlace {
                placement: state.bar_placement(bar)?,
                slot: state.bar_slot(bar),
                pane: None,
            })
        }) else {
            self.end_lift();
            return;
        };
        if target.placement.is_window() {
            self.place_bar(bar, target.placement);
            self.end_lift();
            return;
        }
        let dock = *self;
        self.edit(|state| {
            target.apply(state, bar);
            dock.share_placement(state, bar);
        });
        if lifted {
            self.settle_lift(target);
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

    pub(super) fn bars_at(
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
        let previewed = self.lifting.preview.with(|preview| {
            let preview = preview.as_ref()?;
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
        let target = (!placement.is_window()).then(|| BarPlace {
            placement,
            slot: state.bar_slot(bar),
            pane: None,
        });
        let floated = match placement {
            DockBarPlacement::Over(edge, _) => self
                .bar_areas(bar)
                .map(|(_, pane)| (edge, edge_distance(pane, edge, pointer))),
            _ => None,
        };
        let lifting = self.lifting;
        *lifting.flight.value().borrow_mut() = Flight {
            pos: rect.min,
            speed: Vec2::ZERO,
            pointer,
            grab: pointer - rect.min,
            velocity: Velocity::default(),
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
            target,
            floated,
            settling: false,
            carried: lifting.carried.get().is_some(),
        }));
    }

    pub(super) fn move_lift(&self, point: DragPoint) {
        let point = self.at_home(point);
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
        }
        let target = self
            .resolve_bar(point, lift.bar)
            .filter(|target| !target.placement.is_window());
        let still_over = |edge: DockEdge| {
            target.is_some_and(|target| {
                !target.placement.is_beside() && target.placement.edge() == edge
            })
        };
        if lift.floated.is_some_and(|(edge, _)| !still_over(edge)) {
            self.lifting.lift.set(Some(Lift {
                floated: None,
                ..lift
            }));
        }
        self.retarget(target);
        self.follow_pointer();
        self.animate_lift();
    }

    fn follow_pointer(&self) {
        let lifting = self.lifting;
        let Some(lift) = lifting.lift.get_untracked() else {
            return;
        };
        let flight = lifting.flight.value();
        let mut flight = flight.borrow_mut();
        if let Some(goal) = self.lift_goal(lift, &flight) {
            flight.pos = goal;
            flight.speed = Vec2::ZERO;
            if lifting.at.get_untracked() != goal {
                lifting.at.set(goal);
            }
        }
    }

    fn retarget(&self, target: Option<BarPlace>) {
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

    fn settle_lift(&self, target: BarPlace) {
        let lifting = self.lifting;
        {
            let flight = lifting.flight.value();
            let mut flight = flight.borrow_mut();
            let mut speed = flight.velocity.get();
            // A bar held in a band moved only along it, whatever the pointer did across it.
            if let Some(DockBarPlacement::Beside(edge, _)) = lifting
                .lift
                .get_untracked()
                .and_then(|lift| lift.target)
                .map(|target| target.placement)
            {
                match edge.direction() {
                    Direction::Horizontal => speed.y = 0.0,
                    Direction::Vertical => speed.x = 0.0,
                }
            }
            flight.speed = speed;
        }
        self.retarget(Some(target));
        lifting.lift.update(|lift| {
            if let Some(lift) = lift {
                lift.settling = true;
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

    fn thrown(&self, bar: TabId, point: DragPoint) -> Option<BarPlace> {
        let velocity = self.lifting.flight.value().borrow().velocity.get();
        let (on, body, pane) = self.pane_at(bar, point.pos)?;
        let thrown = point.window == self.home()
            && motion().animates()
            && velocity.length() >= THROW_SPEED;
        if !thrown {
            return None;
        }
        let along = |from: f32, speed: f32, low: f32, high: f32| match speed {
            speed if speed > 0.0 => (high - from) / speed,
            speed if speed < 0.0 => (low - from) / speed,
            _ => f32::INFINITY,
        };
        let across = along(point.pos.x, velocity.x, pane.left(), pane.right());
        let down = along(point.pos.y, velocity.y, pane.top(), pane.bottom());
        let edge = match (across < down, velocity.x > 0.0, velocity.y > 0.0) {
            (true, true, _) => DockEdge::Right,
            (true, false, _) => DockEdge::Left,
            (false, _, true) => DockEdge::Bottom,
            (false, _, false) => DockEdge::Top,
        };
        let hit = point.pos + velocity * across.min(down);
        Some(self.floating_at(bar, on, edge, along_third(body, edge, hit)))
    }

    fn float_near(&self, bar: TabId, point: DragPoint) -> Option<BarPlace> {
        if point.window != self.home() {
            return None;
        }
        let (on, body, pane) = self.pane_at(bar, point.pos)?;
        let (edge, _) = nearest_edge(pane, point.pos);
        Some(self.floating_at(bar, on, edge, along_third(body, edge, point.pos)))
    }

    fn floating_at(
        &self,
        bar: TabId,
        pane: Option<TabId>,
        edge: DockEdge,
        align: DockAlign,
    ) -> BarPlace {
        let placement = DockBarPlacement::Over(edge, align);
        let slot = self
            .stacked_on(bar, pane, placement)
            .iter()
            .filter(|(other, _)| self.same_layer(bar, *other))
            .count();
        BarPlace {
            placement,
            slot,
            pane,
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
        Some(match lift.target.map(|target| target.placement) {
            Some(DockBarPlacement::Beside(edge, _)) => {
                // Until the gap it lands in is measured, the bar keeps its place across the band
                // rather than following the pointer out of it for a frame.
                let held = gap.map_or(flight.pos, |gap| gap.min);
                match edge.direction() {
                    Direction::Horizontal => pos2(free.x, held.y),
                    Direction::Vertical => pos2(held.x, free.y),
                }
            }
            _ => free,
        })
    }

    fn step_lift(&self, frame: AnimationFrame) -> bool {
        let lifting = self.lifting;
        let Some(lift) = lifting.lift.get_untracked() else {
            return false;
        };
        let seconds = frame.delta.as_secs_f32();
        let flight = lifting.flight.value();
        let mut flight = flight.borrow_mut();
        if !lift.settling {
            let pointer = flight.pointer;
            flight.velocity.sample(frame.now, pointer);
        }
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
            let moving = flight.velocity.get().length() >= THROW_SPEED;
            return !resting || growing || moving;
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
    target: Option<BarPlace>,
    /// The edge a bar grabbed floating floated over, and how far from it the pointer grabbed, until
    /// it is carried away from that edge.
    floated: Option<(DockEdge, f32)>,
    settling: bool,
    carried: bool,
}

/// Where a carried bar would land: its placement, its slot among its own layer's bars there, and
/// for a bar that may show on several panes (`DockState::bar_panes`), the tab whose pane it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct BarPlace {
    placement: DockBarPlacement,
    slot: usize,
    pane: Option<TabId>,
}

impl BarPlace {
    fn apply(self, state: &mut DockState, bar: TabId) {
        if let Some(pane) = self.pane {
            state.set_bar_pane(bar, pane);
        }
        state.place_bar_at(bar, self.placement, self.slot);
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Gone {
    place: BarPlace,
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
    carried: StoredCell<Option<Rect>>,
    window_moved: StoredCell<Option<TabId>>,
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
                Some(target) => target.apply(&mut state, lift.bar),
                None => state.set_bar_hidden(lift.bar, true),
            }
            Some(state)
        });
        let left = create_memo(move || {
            let lift = lift.get()?;
            let gone = gone.get()?;
            let mut state = state.get();
            gone.place.apply(&mut state, lift.bar);
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
            carried: StoredCell::new(None),
            window_moved: StoredCell::new(None),
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
        let lift = self.lift.get().filter(|lift| lift.bar == bar)?;
        Some(match lift.target {
            Some(target) => target.placement,
            None => DockBarPlacement::Window {
                edge: lift.edge,
                origin: Pos2::ZERO,
            },
        })
    }
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

fn edge_distance(pane: Rect, edge: DockEdge, pos: Pos2) -> f32 {
    match edge {
        DockEdge::Top => pos.y - pane.top(),
        DockEdge::Bottom => pane.bottom() - pos.y,
        DockEdge::Left => pos.x - pane.left(),
        DockEdge::Right => pane.right() - pos.x,
    }
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
            <ListItem grow=1.0>
                <List direction=Direction::Horizontal spacing=0.0>
                    <DockBarBand dock holder edge=DockEdge::Left />
                    <ListItem grow=1.0>
                        <Layers @node_ref=&body>
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
                            <DockBarFloat
                                dock
                                holder
                                edge=DockEdge::Top
                                align=DockAlign::End
                                ends={top}
                            />
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
                            <Show condition={matches!(holder, BarHolder::Dock)}>
                                <Layers>
                                    <DockBarHidden dock edge=DockEdge::Top />
                                    <DockBarHidden dock edge=DockEdge::Bottom />
                                    <DockBarHidden dock edge=DockEdge::Left />
                                    <DockBarHidden dock edge=DockEdge::Right />
                                </Layers>
                            </Show>
                        </Layers>
                    </ListItem>
                    <DockBarBand dock holder edge=DockEdge::Right />
                </List>
            </ListItem>
            <DockBarBand dock holder edge=DockEdge::Bottom />
        </List>
    }
}

#[component]
fn DockBarBand(dock: State, holder: BarHolder, edge: DockEdge) -> NodeId {
    let bars = dock.bars_at(holder, edge, false, move |placement| {
        placement.is_beside() && placement.edge() == edge
    });
    let concealed = create_memo(move || matches!(holder, BarHolder::Dock) && dock.conceals(edge));
    let shown = create_memo(move || match bars.with(Vec::is_empty) || concealed.get() {
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
    above_embeds(
        band,
        create_memo(move || {
            (dock.bar_level() == LayerLevel::Raised).then_some(AboveEmbeds::Raised)
        }),
    );
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
pub(super) fn DockBarRun(
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
                        <ListItem shrink=1.0>
                            <DockBarSlot dock bar placement />
                        </ListItem>
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
                    <ListItem shrink=1.0>
                        <DockBarSlot dock bar placement />
                    </ListItem>
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
    let lifting = dock.lifting;
    let lifted_from = create_memo(move || {
        let bar = lifting
            .lift
            .with(|lift| lift.filter(|lift| !lift.carried).map(|lift| lift.bar))?;
        dock.state
            .with(|state| state.bar_placement(bar))
            .is_some_and(DockBarPlacement::is_window)
            .then(|| dock.window_key(bar))
    });
    let keys = create_memo(move || {
        if away.get() {
            return Vec::new();
        }
        let mut keys = dock.shown(|state| {
            state
                .bar_windows(&|bar| dock.window_key(bar))
                .into_iter()
                .filter(|(_, bars)| state.followed_bar(bars).is_some())
                .map(|(key, _)| key)
                .collect::<Vec<_>>()
        });
        if let Some(key) = lifted_from.get()
            && !keys.contains(&key)
        {
            keys.push(key);
        }
        keys
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
    let held = key.clone();
    let kept = create_memo(move || {
        state.with(|state| {
            let windows = state.bar_windows(&|bar| dock.window_key(bar));
            let (_, bars) = windows.iter().find(|(key, _)| *key == held)?;
            state.followed_bar(bars)
        })
    });
    let origin = create_memo(move || {
        let placement = followed
            .get()
            .or_else(|| kept.get())
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
    let arriving = create_memo(move || {
        !left
            && lifting.lift.with(|lift| {
                lift.is_some_and(|lift| {
                    lift.bar == bar
                        && lift
                            .target
                            .is_some_and(|target| target.placement.same_place(placement))
                })
            })
    });
    let lifted = create_memo(move || {
        left || lifting
            .lift
            .with(|lift| lift.is_some_and(|lift| lift.bar == bar))
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
    let node = create_signal(None);
    create_effect(move || {
        node.set(Some(match lifted.get() {
            true => gap,
            false => dock.bar_panel(bar),
        }));
    });
    view! {
        <Portal node={node} />
    }
}

#[component]
pub(super) fn DockBarLifted(dock: State) -> NodeId {
    let lifting = dock.lifting;
    let lifted = create_memo(move || lifting.lift.with(|lift| lift.map(|lift| lift.bar)));
    let node = create_signal(None);
    create_effect(move || node.set(lifted.get().map(|bar| dock.bar_panel(bar))));
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
            placement={Placement::Over(0)}
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
