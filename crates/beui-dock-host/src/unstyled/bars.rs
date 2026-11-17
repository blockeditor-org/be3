//! Tool bars (`DockBar`): bands, floating runs and popped-out windows, and carrying a bar the user
//! drags. A carried bar is a carry on the drag board (plans/declarative_dragging.md, step 4): the
//! dock is a drop site whose places are the slots of every band the bar may dock in, read from the
//! bars' laid-out rects, and, away from them, wherever it floats: `closest(band slots, sticky)
//! .when_far(float).thrown()`. The bands draw the chosen place from the site's `over`, the bars
//! beside it springing aside, and the carried bar is drawn in its own `LayerPolicy::Inert` layer
//! at the bars' level. Each run of floating dock bars is its own layer at `LayerLevel::Raised`:
//! floating dock windows pass under it, menus and modals over it.

use be_lib_stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use beui_components_unstyled::{Glide, drag_board};
use beui_macros::{component, view};
use beui_view::components::overlay::{Overlay, OverlaysWithin};
use beui_view::components::root::Root;
use beui_view::drag::{
    Candidate, Carry, DragPoint, DragSpec, Driver, closest, custom, floating,
};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::input::PointerId;
use beui_view::types::window::{WindowId, WindowPlacement, WindowReport};
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
    DockAlign, DockBarPlacement, DockDragged, DockEdge, DockMode, DockState, Entry,
    FLOAT_INSET, Held, LeafId, State, TabId,
};

const WINDOW_ROOM: f32 = 4096.0;
const WINDOW_SIZE: Vec2 = Vec2::new(240.0, 40.0);
const POP_OFFSET: Vec2 = Vec2::new(16.0, 16.0);
const BAND_SHARE: f32 = 0.12;
const BAND_MIN: f32 = 24.0;
const BAND_MAX: f32 = 56.0;
/// How near an edge, in bars (the carried bar's thickness), the pointer docks a bar in that
/// edge's band.
const DOCK_REACH: f32 = 1.0;
/// How far out, in bars, a band the pointer has reached holds on to it.
const DOCK_HOLD: f32 = 1.5;
/// A bar grabbed while floating over an edge docks there only once the pointer has come this
/// share of the way from where it grabbed towards the edge, so a grab alone keeps it floating.
const FLOAT_HOLD: f32 = 0.5;

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
        self.panels.is_alive() && self.carried() == Some(DockDragged::Bar(bar))
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

    fn carry(&self, bar: TabId, down: PointerPress) -> Option<Carry> {
        let dock = *self;
        if !dock.panels.is_alive() || dock.mode.get_untracked() != DockMode::Tiled {
            return None;
        }
        // A bar in a window of its own that moves itself moves that window.
        if dock.moves_bar_window(bar) {
            return None;
        }
        let rect = dock.bar_home_rect(bar)?;
        let down = dock.at_home(DragPoint::of(down));
        Some(dock.lift_carry(bar, rect, down.pos - rect.min, false))
    }
}

/// Any node's paint order and window, for a carry the dock relays: the board notes only the
/// windows, and every place of a bar is in the dock's.
fn order_of(node: NodeId) -> (Vec<usize>, WindowId) {
    (Vec::new(), window_of(node))
}

impl State {
    /// Builds `bar`'s panel, in the scope of the `Kept` that keeps it (`Held::Bar`).
    pub(super) fn build_bar_panel(&self, bar: TabId) -> NodeId {
        let dock = *self;
        provide_kept_shown(self.shown);
        let panel = {
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
            // A menu or panel open from the bar keeps its hidden band out from under it.
            let within = OverlaysWithin::new();
            provide_context(within);
            let reveal = dock.reveal;
            create_effect(move || {
                let held = within.any();
                untrack(|| reveal.hold(bar, held));
            });
            on_cleanup(move || reveal.hold(bar, false));
            self.content.call(bar)
        };
        self.bar_panels.value().borrow_mut().insert(bar, panel);
        let panels = self.bar_panels;
        on_cleanup(move || {
            if panels.is_alive() {
                panels.value().borrow_mut().remove(&bar);
            }
        });
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

    /// A bar an app's grip carries across the app protocol: `moved.at` is the pointer in the
    /// bar as it was laid out when the carry began.
    fn carry_bar(&self, bar: TabId, moved: DockBarMove) {
        if self.lifting.window_moved.get() == Some(bar) {
            if moved.stage != DockDragStage::Move {
                self.lifting.window_moved.set(None);
            }
            return;
        }
        let relayed = self.relayed.get_untracked().filter(|(held, _)| *held == bar);
        let rect = match relayed {
            Some((_, rect)) => Some(rect),
            None => self.bar_home_rect(bar),
        };
        let Some(rect) = rect else {
            return;
        };
        let mut point = DragPoint::at(rect.min + moved.at.to_vec2(), Default::default());
        point.window = self.home();
        match moved.stage {
            DockDragStage::Move if relayed.is_some() => self.relay(point),
            DockDragStage::Move => {
                if self.mode.get_untracked() != DockMode::Tiled || self.relayed.get_untracked().is_some() {
                    return;
                }
                if self.moves_bar_window(bar) {
                    self.lifting.window_moved.set(Some(bar));
                    return;
                }
                self.relay_lift(bar, rect, moved.at.to_vec2(), point);
            }
            DockDragStage::Drop if relayed.is_some() => {
                self.relay(point);
                self.relay_end(false);
            }
            DockDragStage::Cancel if relayed.is_some() => self.relay_end(true),
            DockDragStage::Drop | DockDragStage::Cancel => {}
        }
    }

    /// The carry of `bar`, laid out at `rect` (in the dock's window) and held `grab` from its
    /// top left corner. The item the board carries is a square as long as the bar, centred on
    /// the pointer: a band then chooses by how near the pointer comes across it, whichever way
    /// the bar lies, and its slot is drawn under the pointer along it (`following`). The dock
    /// draws the bar itself (`DockBarLifted`).
    fn lift_carry(&self, bar: TabId, rect: Rect, grab: Vec2, hosted: bool) -> Carry {
        let dock = *self;
        let state = self.state.get_untracked();
        let placement = state.bar_placement(bar);
        let edge = placement.map_or(DockEdge::Top, DockBarPlacement::edge);
        let floated = match placement {
            Some(DockBarPlacement::Over(edge, _)) => self
                .bar_areas(bar)
                .map(|(_, pane)| (edge, edge_distance(pane, edge, rect.min + grab))),
            _ => None,
        };
        self.lifting.lift.set(Some(Lift {
            bar,
            edge,
            grab,
            size: rect.size(),
            from: placement.filter(|placement| state.is_dock_bar(bar) && !placement.is_beside()),
            floated,
            hosted,
        }));
        let length = rect.width().max(rect.height());
        Carry::new(self.carry(DockDragged::Bar(bar)))
            .drawn_by_site()
            .kept_in(self.home())
            .item(Rect::from_center_size(Pos2::ZERO, Vec2::splat(length)))
            .outside(move |point| dock.bar_outside(bar, point))
    }

    /// Lifts `bar` for a carry the dock relays (`PointerId::Relayed`) for an app's grip, at
    /// `point` in its window.
    fn relay_lift(&self, bar: TabId, rect: Rect, grab: Vec2, point: DragPoint) {
        let carry = self.lift_carry(bar, rect, grab, true);
        let point = DragPoint {
            pointer: PointerId::Relayed,
            ..point
        };
        if drag_board().lift(carry, point).is_some() {
            self.relayed.set(Some((bar, rect)));
        }
    }

    fn relay(&self, point: DragPoint) {
        let point = DragPoint {
            pointer: PointerId::Relayed,
            ..point
        };
        Driver::track(&*drag_board(), point, &order_of);
    }

    /// Lets go of the carry the dock relays: dropped where it was last moved, or put back.
    fn relay_end(&self, cancelled: bool) {
        self.relayed.set(None);
        Driver::release(&*drag_board(), PointerId::Relayed, cancelled, &order_of);
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

    fn bar_panel_rect(&self, bar: TabId) -> Option<Rect> {
        let panel = *self.bar_panels.value().borrow().get(&bar)?;
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

    /// The panes a carried `bar` may land on, each with the tab naming it (none for the dock's
    /// own bars) and its body and pane rects: the dock for its own bars, else whichever of
    /// the panes it may show on (`DockState::bar_panes`) are laid out, its own first.
    fn bar_panes(&self, bar: TabId) -> Vec<(Option<TabId>, Rect, Rect)> {
        let state = self.state.get_untracked();
        if state.is_dock_bar(bar) {
            return self
                .bar_areas(bar)
                .map(|(body, pane)| (None, body, pane))
                .into_iter()
                .collect();
        }
        let own = state.bar_tab(bar);
        own.into_iter()
            .chain(
                state
                    .bar_panes(bar)
                    .into_iter()
                    .filter(|tab| Some(*tab) != own),
            )
            .filter_map(|tab| {
                let (body, pane) = self.pane_areas(tab)?;
                Some((Some(tab), body, pane))
            })
            .collect()
    }

    /// Of the panes a carried `bar` may land on, the first holding `pos`.
    fn pane_at(&self, bar: TabId, pos: Pos2) -> Option<(Option<TabId>, Rect, Rect)> {
        self.bar_panes(bar)
            .into_iter()
            .find(|(_, _, pane)| pane.contains(pos))
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

    /// Where a carried bar can land, read at the lift from where the bars are laid out: each
    /// slot of each band of each pane it may land on (`band_slots`), open while the pointer
    /// is over that pane within `DOCK_REACH` of its edge, the slot nearest along the band
    /// chosen and held until another is clearly nearer; farther from every band it floats
    /// (`float_at`); and a release at speed carries on along its path before choosing. A bar
    /// grabbed while floating over an edge reaches that edge's band only once pulled halfway
    /// from where it was grabbed towards the edge.
    pub(super) fn bar_spec(&self, bar: TabId, tiles: NodeRef) -> DragSpec<BarPlace> {
        let Some(lift) = self.lifting.lift.get_untracked().filter(|lift| lift.bar == bar) else {
            return floating();
        };
        let Some(origin) = tiles
            .try_get()
            .and_then(|node| measure(node).get_untracked())
            .map(|measured| measured.rect.min)
        else {
            return floating();
        };
        let own = self.state.with_untracked(|state| state.bar_tab(bar));
        let thickness = lift.size.x.min(lift.size.y);
        let mut bands = Vec::new();
        for (tab, body, pane) in self.bar_panes(bar) {
            for edge in DockEdge::ALL {
                let slots = self.band_slots(bar, tab, edge, (body, pane), lift.size);
                let slots: Vec<Candidate<BarPlace>> = slots
                    .into_iter()
                    .map(|(place, rest, label)| {
                        Candidate::new(place, rest.translate(-origin.to_vec2()), tiles)
                            .labelled(label)
                    })
                    .collect();
                if slots.is_empty() {
                    continue;
                }
                let floated = lift
                    .floated
                    .filter(|(over, _)| *over == edge && tab == own);
                // Reached within `DOCK_REACH` of the edge, from a little past it too, where a
                // pointer carrying a bar to the edge of the window may end up, and held on to
                // out to `DOCK_HOLD` once reached.
                let reached = pane.expand(thickness);
                let held = Rc::new(std::cell::Cell::new(false));
                bands.push(closest(slots).following(edge.direction()).open_where(
                    move |point: DragPoint| {
                        let from_edge = edge_distance(pane, edge, point.pos);
                        let reach = match held.get() {
                            true => DOCK_HOLD,
                            false => DOCK_REACH,
                        } * thickness;
                        let open = reached.contains(point.pos)
                            && from_edge <= reach
                            && floated
                                .is_none_or(|(_, grabbed)| from_edge <= grabbed * FLOAT_HOLD);
                        held.set(open);
                        open
                    },
                ));
            }
        }
        let dock = *self;
        let float = custom(tiles, move |local: Pos2| {
            let tiles = measure(tiles.try_get()?).get_untracked()?.rect;
            Some(dock.float_at(bar, tiles.min + local.to_vec2()))
        });
        // A band's reach is where it is open, so the bands hold the bar wherever one is;
        // only a floating bar is thrown: let go at speed in a band, it docks there. With
        // motion reduced nothing is thrown, as nothing would be seen flying.
        let float = match motion().animates() {
            true => float.thrown(),
            false => float,
        };
        closest(bands)
            .when_far(float)
            .with_distance(f32::INFINITY)
    }

    /// The slots of `edge`'s band of a pane (`areas`: its body and its whole rect, bands
    /// included), each with where a bar of `size` rests there, in the window's points, and
    /// what a screen reader says for it. A run of bars (`Beside(edge, align)`) gives a slot at
    /// each gap between its bars and at its ends, each resting across the gap, so the slot
    /// chosen is the gap nearest the pointer; an empty run gives one where the bar would sit
    /// alone. Across, each rests in a band a bar thick along the pane's edge.
    fn band_slots(
        &self,
        bar: TabId,
        pane: Option<TabId>,
        edge: DockEdge,
        (body, area): (Rect, Rect),
        size: Vec2,
    ) -> Vec<(BarPlace, Rect, String)> {
        let (length, thickness) = (size.x.max(size.y), size.x.min(size.y));
        let rest = |along: f32| {
            let (start, end) = (along - length / 2.0, along + length / 2.0);
            match edge {
                DockEdge::Top => Rect::from_min_max(
                    pos2(start, area.top()),
                    pos2(end, area.top() + thickness),
                ),
                DockEdge::Bottom => Rect::from_min_max(
                    pos2(start, area.bottom() - thickness),
                    pos2(end, area.bottom()),
                ),
                DockEdge::Left => Rect::from_min_max(
                    pos2(area.left(), start),
                    pos2(area.left() + thickness, end),
                ),
                DockEdge::Right => Rect::from_min_max(
                    pos2(area.right() - thickness, start),
                    pos2(area.right(), end),
                ),
            }
        };
        let named = match pane {
            Some(tab) => format!(
                "the {} band of {}",
                edge_name(edge),
                self.entry_title(Entry::Tab(tab))
            ),
            None => format!("the {} band", edge_name(edge)),
        };
        let (start, end) = (along_start(edge, body), along_end(edge, body));
        let mut slots = Vec::new();
        for align in DockAlign::ALL {
            let placement = DockBarPlacement::Beside(edge, align);
            let lined = self.stacked_on(bar, pane, placement);
            let gaps: Vec<f32> = match lined.as_slice() {
                [] => vec![match align {
                    DockAlign::Start => start + length / 2.0,
                    DockAlign::Center => (start + end) / 2.0,
                    DockAlign::End => end - length / 2.0,
                }],
                lined => std::iter::once(along_start(edge, lined[0].1))
                    .chain(lined.windows(2).map(|pair| {
                        (along_end(edge, pair[0].1) + along_start(edge, pair[1].1)) / 2.0
                    }))
                    .chain(std::iter::once(along_end(edge, lined[lined.len() - 1].1)))
                    .collect(),
            };
            for (slot, along) in gaps.into_iter().enumerate() {
                slots.push((
                    BarPlace {
                        placement,
                        slot,
                        pane,
                        shown: true,
                    },
                    rest(along),
                    named.clone(),
                ));
            }
        }
        slots
    }

    /// Where a bar let go at `pos` (in the dock's window) away from every band floats: over the
    /// edge of the pane under it that it is nearest, in the third of that edge it is in and
    /// among the bars floating there by how far in it is; outside every pane it may land on, in
    /// a window of its own, held where it was grabbed. The place floating over an edge shows
    /// while carried only within two bands of that edge, or of the bars floating there; farther
    /// in, the bar floats as it was until let go. A point past the dock (a throw's path runs
    /// on out of it) is taken where it leaves the dock.
    fn float_at(&self, bar: TabId, pos: Pos2) -> BarPlace {
        let area = self.rect.get_untracked();
        let pos = pos2(
            pos.x.clamp(area.left(), area.right() - 1.0),
            pos.y.clamp(area.top(), area.bottom() - 1.0),
        );
        if let Some((on, body, pane)) = self.pane_at(bar, pos) {
            let (edge, distance) = nearest_edge(pane, pos);
            let placement = DockBarPlacement::Over(edge, along_third(body, edge, pos));
            let stacked = self.stacked_on(bar, on, placement);
            let slot = stacked
                .iter()
                .filter(|(_, rect)| centre_reach(pane, *rect, edge) < distance)
                .count();
            let over = stacked
                .iter()
                .map(|(_, rect)| reach_far(pane, *rect, edge))
                .fold(0.0_f32, f32::max);
            let band = band(body);
            return BarPlace {
                placement,
                slot,
                pane: on,
                shown: distance < (reach(pane, body, edge) + band * 2.0).max(over + band),
            };
        }
        let lift = self.lifting.lift.get_untracked();
        let (grab, size, edge) = lift.map_or((Vec2::ZERO, WINDOW_SIZE, DockEdge::Top), |lift| {
            (lift.grab, lift.size, lift.edge)
        });
        let dock = self.rect.get_untracked();
        let origin = pos - dock.min.to_vec2() - grab;
        let origin = self.clamped_origin(Rect::from_min_size(origin, size), dock.size());
        BarPlace {
            placement: DockBarPlacement::Window { edge, origin },
            slot: 0,
            pane: None,
            shown: false,
        }
    }

    /// Drops a carried bar on `place`: its slot glides there from where it was drawn.
    pub(super) fn commit_bar(&self, bar: TabId, place: BarPlace) {
        if place.placement.is_window() {
            self.place_bar(bar, place.placement);
            return;
        }
        if let Some(drawn) = self.lifting.drawn.get() {
            self.lifting.landing.set(Some((bar, drawn)));
        }
        let dock = *self;
        self.edit(|state| {
            place.apply(state, bar);
            dock.share_placement(state, bar);
        });
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

    pub(super) fn bars_at(
        &self,
        holder: BarHolder,
        edge: DockEdge,
        stacked: bool,
        wanted: impl Fn(DockBarPlacement) -> bool + 'static,
    ) -> Memo<Vec<TabId>> {
        let away = self.bars_away;
        let dock = *self;
        create_memo(move || {
            let tab = match holder {
                BarHolder::Tab { tab, .. } => match tab.get().filter(|_| !away.get()) {
                    Some(tab) => Some(tab),
                    None => return Vec::new(),
                },
                BarHolder::Dock => None,
            };
            let mut bars = dock.shown(|state: &DockState| -> Vec<TabId> {
                let held = match tab {
                    Some(tab) => state.pane_bars(tab),
                    None => state.bars_of(None),
                };
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
}

/// The bar a carry lifted and how: where it was grabbed, its size, the edge it was on, the
/// floating placement of the dock's own bar it left (`DockBarOverlay` keeps that open), and,
/// grabbed floating over an edge, that edge and how far from it the pointer grabbed, and
/// whether an app's grip carries it across the app protocol.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Lift {
    bar: TabId,
    edge: DockEdge,
    grab: Vec2,
    size: Vec2,
    from: Option<DockBarPlacement>,
    floated: Option<(DockEdge, f32)>,
    hosted: bool,
}

/// Where a carried bar would land: its placement, its slot among the bars there, for a bar
/// that may show on several panes (`DockState::bar_panes`) the tab whose pane it is, and
/// whether the dock shows it there while it is carried (a window, or a floating place far
/// from its edge, only takes it when let go).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct BarPlace {
    placement: DockBarPlacement,
    slot: usize,
    pane: Option<TabId>,
    shown: bool,
}

impl BarPlace {
    fn apply(self, state: &mut DockState, bar: TabId) {
        state.place_bar_on(bar, self.pane, self.placement, self.slot);
    }
}

#[derive(Clone, Copy)]
pub(super) struct Lifting {
    /// What the bar site chose for the carried bar (its `over`).
    pub(super) over: Signal<Option<ReadSignal<Option<BarPlace>>>>,
    /// The bar the dock's carry lifted, while the board carries it.
    lifted: Memo<Option<Lift>>,
    lift: Signal<Option<Lift>>,
    /// Where the carried bar was drawn last, in the dock's window.
    drawn: Signal<Option<Pos2>>,
    /// A bar just dropped and where it was drawn, which its slot glides from.
    landing: Signal<Option<(TabId, Pos2)>>,
    window_moved: StoredCell<Option<TabId>>,
    preview: Memo<Option<DockState>>,
}

impl Lifting {
    pub(super) fn new(state: Binding<DockState>, carried: Memo<Option<TabId>>) -> Self {
        let lift = create_signal(None::<Lift>);
        let over = create_signal(None::<ReadSignal<Option<BarPlace>>>);
        let lifted = create_memo(move || {
            let bar = carried.get()?;
            lift.get().filter(|lift| lift.bar == bar)
        });
        let preview = create_memo(move || {
            let lift = lifted.get()?;
            let mut state = state.get();
            match over.get().and_then(|over| over.get()) {
                Some(place) if place.shown => place.apply(&mut state, lift.bar),
                _ => state.set_bar_hidden(lift.bar, true),
            }
            Some(state)
        });
        Self {
            over,
            lifted,
            lift,
            drawn: create_signal(None),
            landing: create_signal(None),
            window_moved: StoredCell::new(None),
            preview,
        }
    }

    /// The placement `bar` takes while carried: the chosen place's, else floating.
    fn looks(&self, bar: TabId) -> Option<DockBarPlacement> {
        let lift = self.lifted.get().filter(|lift| lift.bar == bar)?;
        let over = self.over.get().and_then(|over| over.get());
        Some(match over {
            Some(place) if place.shown => place.placement,
            _ => DockBarPlacement::Window {
                edge: lift.edge,
                origin: Pos2::ZERO,
            },
        })
    }
}

/// How deep a band of a body this size is, for how near its edge a floating bar shows there.
fn band(body: Rect) -> f32 {
    (body.width().min(body.height()) * BAND_SHARE).clamp(BAND_MIN, BAND_MAX)
}

fn edge_name(edge: DockEdge) -> &'static str {
    match edge {
        DockEdge::Top => "top",
        DockEdge::Bottom => "bottom",
        DockEdge::Left => "left",
        DockEdge::Right => "right",
    }
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
                            {(matches!(holder, BarHolder::Dock)).then(|| view! {
                                <Layers>
                                    <DockBarHidden dock edge=DockEdge::Top />
                                    <DockBarHidden dock edge=DockEdge::Bottom />
                                    <DockBarHidden dock edge=DockEdge::Left />
                                    <DockBarHidden dock edge=DockEdge::Right />
                                </Layers>
                            })}
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
    let band = view! {
        <List direction={across(edge)} spacing=0.0>
            <Show value={move || !bars.with(Vec::is_empty) && !concealed.get()}>|| {
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
            }</Show>
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
    band
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
                <ForEach value={bars}>|bar: TabId| {
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
            padding.set(sides);
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
            <ForEach value={bars}>|bar: TabId| {
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
            .lifted
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
    // A bar lifted out of its window keeps that window while carried, as the press carrying
    // it began there.
    let lifted_from = create_memo(move || {
        let bar = lifting
            .lifted
            .with(|lift| lift.filter(|lift| !lift.hosted).map(|lift| lift.bar))?;
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
            <ForEach value={keys}>|key: String| {
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
        if measured.x > 0.0 && measured.y > 0.0 {
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
                        <ForEach value={bars}>|bar: TabId| {
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
    let held = view! {
        <Kept key={Held::Bar(bar)}>|| { dock.build_bar_panel(bar) }</Kept>
    };
    let panel = *dock
        .bar_panels
        .value()
        .borrow()
        .get(&bar)
        .expect("a kept bar's panel is built");
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
    let room = create_memo(move || match shown.get() {
        true => WINDOW_ROOM,
        false => 0.0,
    });
    view! {
        <CanvasItem
            x=0.0
            y=0.0
            width={room}
            height={room}
            clip={move || !shown.get()}
            pointer={shown}
        >
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=WINDOW_ROOM height=WINDOW_ROOM clip=false>
                    <Frame align=Align2::START>{held}</Frame>
                </CanvasItem>
            </Canvas>
        </CanvasItem>
    }
}

/// One bar in a run or floating line, gliding from where it was drawn to where its line puts
/// it whenever a carried bar's place changes (the bars beside a slot moving aside for it) and,
/// just dropped, from where the carried bar was drawn. While carried, it is the gap it would
/// fill there, the size it was lifted at, turned to lie along its line.
#[component]
fn DockBarSlot(dock: State, bar: TabId, placement: DockBarPlacement) -> NodeId {
    let lifting = dock.lifting;
    let lifted = create_memo(move || {
        lifting
            .lifted
            .with(|lift| lift.filter(|lift| lift.bar == bar).map(|lift| lift.size))
    });
    let gap = create_memo(move || {
        let size = lifted.get().unwrap_or(Vec2::ZERO);
        let (long, short) = (size.x.max(size.y), size.x.min(size.y));
        match placement.edge().direction() {
            Direction::Horizontal => vec2(long, short),
            Direction::Vertical => vec2(short, long),
        }
    });
    let width = create_memo(move || gap.get().x);
    let height = create_memo(move || gap.get().y);
    let from = create_memo(move || {
        lifting
            .landing
            .get()
            .filter(|(landed, _)| *landed == bar)
            .map(|(_, at)| at)
    });
    let shown = view! {
        <Show
            value={move || lifted.get().is_some()}
            fallback={move || view! {
                <DockBarPanel dock bar={Some(bar)} />
            }}
        >
            <Frame width=width height=height />
        </Show>
    };
    let glided = view! {
        <Glide spring={dock.curves.settle} on={lifting.preview} from={from}>{shown}</Glide>
    };
    // The glide has taken where to start from; a slot built later starts where it is laid out.
    create_effect(move || {
        if from.get().is_some() {
            untrack(|| lifting.landing.set(None));
        }
    });
    glided
}

/// The carried bar, drawn where the carry is: on a band's slot, across the band, held along it
/// where it was grabbed; elsewhere under the pointer as it was grabbed.
#[component]
pub(super) fn DockBarLifted(dock: State) -> NodeId {
    let lifting = dock.lifting;
    let board = drag_board();
    let lifted = create_memo(move || lifting.lifted.with(|lift| lift.map(|lift| lift.bar)));
    let held = view! {
        <DockBarPanel dock bar={lifted} />
    };
    let at = create_memo(move || {
        let lift = lifting.lifted.get()?;
        let frame = board.frame()?;
        let pointer = frame.item.center();
        let over = lifting.over.get().and_then(|over| over.get());
        let long = |grab: Vec2| match lift.size.x >= lift.size.y {
            true => grab.x,
            false => grab.y,
        };
        Some(match (frame.floating, over.map(|place| place.placement)) {
            (false, Some(DockBarPlacement::Beside(edge, _))) => match edge.direction() {
                Direction::Horizontal => pos2(pointer.x - long(lift.grab), frame.item.min.y),
                Direction::Vertical => pos2(frame.item.min.x, pointer.y - long(lift.grab)),
            },
            _ => pointer - lift.grab,
        })
    });
    create_effect(move || {
        if let Some(at) = at.get() {
            untrack(|| lifting.drawn.set(Some(at)));
        } else if lifted.with(Option::is_none) {
            untrack(|| lifting.drawn.set(None));
        }
    });
    let anchor = create_memo(move || OverlayAnchor::Point(at.get().unwrap_or(Pos2::ZERO)));
    let open = create_memo(move || at.with(Option::is_some));

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

/// The bars' panels built now, kept by the dock's `KeepAlive` (`Held::Bar`).
pub(super) type BarPanels = RefCell<HashMap<TabId, NodeId>>;

/// `bar`'s panel, kept built and paused while no `DockBarPanel` shows it.
#[component]
fn DockBarPanel(dock: State, bar: Prop<Option<TabId>>) -> NodeId {
    view! {
        <Dynamic value={bar} fallback=()>|bar: TabId| {
            view! {
                <Kept key={Held::Bar(bar)}>|| { dock.build_bar_panel(bar) }</Kept>
            }
        }</Dynamic>
    }
}

pub(crate) type BarCovers = HashMap<(TabId, DockEdge, DockAlign), Rect>;

pub(crate) type BarFloats = HashMap<(DockEdge, DockAlign), Rect>;
