//! The fluid preview (plans/declarative_dragging.md, step 3b): a carried tab or pane chooses
//! among the places the dock's layout offers, each carrying the state after its drop; the
//! dock lays its panes out as that state would and springs between layouts, panels keep the
//! size they were laid out at until the drop commits, and the carried item is drawn the shape
//! of where it would rest, gliding there on release. `DockState` is untouched until then.

use std::rc::Rc;

use be_lib_stable_hash::HashMap;
use beui_components_unstyled::drag_board;
use beui_components_unstyled::{DragPoint, ReorderGap};
use beui_macros::{component, view};
use beui_view::drag::{Candidate, DragSpec, closest, custom};
use beui_view::measure::measure;
use beui_view::primitives::*;
use beui_view::reactive::*;
use beui_view::types::window::WindowId;
use beui_view::types::{
    CanvasArrangement, Direction, Measured, Modifiers, Pos2, Rect, Spring, Vec2,
};

use beui_dock_state::*;

use super::{
    DockDragged, DockPreviewHandle, EDGE_ZONE, GRAB_OFFSET, OUTER_EDGE, State, arranged, leaf_key,
    middle_target, split_key,
};

/// A place a carried tab or pane can land in the dock: the drop, the state it leaves, the
/// surface whose layout offers it, and where the carried item rests there, in that surface's
/// window's points. Two places are the same place when their drops and surfaces are: `after`
/// is the state at the lift, which the preview keeps, while the commit replays `drop` on the
/// state as it is at the release.
#[derive(Clone)]
pub(super) struct DockPlace {
    pub(super) drop: DockDrop,
    pub(super) after: Rc<DockState>,
    pub(super) surface: Option<SurfaceId>,
    pub(super) rest: Option<Rect>,
}

impl PartialEq for DockPlace {
    fn eq(&self, other: &Self) -> bool {
        self.drop == other.drop && self.surface == other.surface
    }
}

/// A dropped tab or pane gliding into where it landed: from where it was drawn last to its
/// rest, in `surface`'s window's points.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Landing {
    surface: SurfaceId,
    dragged: DockDragged,
    from: Rect,
    to: Rect,
}

impl State {
    /// Where a carried tab or pane can land: for each pane of each surface laid out now, a
    /// split on each edge, grouping in its middle, and the places of a group it hosts, each
    /// chosen by a zone (bands along the edges, the middle) that leaves the pane's tab bar to
    /// the bar, a `Reorder` that parts for it. Only places the state admits are offered, so a
    /// locked place opens nothing. A surface's places are reached only where none of the
    /// dock's other windows or dialogs, nor anything else, covers it (`reaches`). With Alt
    /// held the item floats in a window of its own at the pointer, and those are the only
    /// places.
    pub(super) fn spec(&self, dragged: DockDragged, tiles: NodeRef) -> DragSpec<DockPlace> {
        let dock = *self;
        let state = self.state.get_untracked();
        // Topmost window first: of zones holding the pointer, the first listed wins.
        let mut surfaces = state.windows();
        surfaces.reverse();
        surfaces.push(state.main());
        let mut places = Vec::new();
        // The surfaces that offer a place. The topmost surface under the pointer is the one
        // whose places it reaches; over one that offers none (the window a lone tab is
        // carried from) the item floats there, which moves that window.
        let offering: Rc<std::cell::RefCell<Vec<SurfaceId>>> = Rc::default();
        for surface in surfaces {
            let window = self.windows.value().borrow().get(&surface).copied();
            let (node, split) = match window {
                Some(window) => (window, false),
                None if surface == state.main() => (tiles, true),
                None => continue,
            };
            let Some(origin) = node
                .try_get()
                .and_then(|node| measure(node).get_untracked())
                .map(|measured| measured.rect.min)
            else {
                continue;
            };
            let mut candidates = Vec::new();
            let mut zones = Zones {
                state: &state,
                dragged,
                surface,
                origin,
                node,
                out: &mut candidates,
            };
            // A window of one pane is that pane wherever the pointer is in it, its own bar
            // included, as the pane's own tab bar is the bar's.
            let whole = window
                .and_then(|window| window.try_get())
                .and_then(|window| measure(window).get_untracked())
                .map(|measured| measured.rect);
            self.zones_in(&mut zones, Tree::Surface(surface), split, whole);
            if candidates.is_empty() {
                continue;
            }
            offering.borrow_mut().push(surface);
            places.push(
                closest(candidates)
                    .shaped(node)
                    .open_where(move |point| dock.reaches(node, surface, point)),
            );
        }
        let floats = move |anywhere: bool| {
            let offering = Rc::clone(&offering);
            custom(tiles, move |local: Pos2, _| {
                let (window, pos) = dock.open_at(tiles, local)?;
                let topmost = dock.topmost(window, pos);
                let offers = topmost.is_some_and(|top| offering.borrow().contains(&top));
                if window != dock.home() || (!anywhere && offers) {
                    return None;
                }
                let area = dock.rect.get_untracked();
                let origin = pos - area.min.to_vec2() - GRAB_OFFSET;
                let origin =
                    dock.clamped_origin(Rect::from_min_size(origin, FLOATING_SIZE), area.size());
                let state = dock.state.get_untracked();
                dock.place(&state, dragged, DockDrop::Window { pos: origin }, None)
            })
        };
        // Over no surface of the home window, or a surface offering no place, the item floats
        // in a window of its own there; listed last, so every surface's places come first.
        places.push(floats(false));
        closest(places).when_held(Modifiers::ALT, floats(true))
    }

    /// The places of `tree`'s panes, inner ones (a group a pane hosts) before the pane's own.
    fn zones_in(&self, zones: &mut Zones, tree: Tree, split: bool, whole: Option<Rect>) {
        let Some(area) = self.pane_rect(tree) else {
            return;
        };
        let layout = layout_tree(zones.state, tree, area, self.thickness);
        let alone = layout.leaves.len() == 1;
        for (leaf, rect) in layout.leaves {
            let body = match whole.filter(|_| alone) {
                Some(whole) => self.body_of(zones.state, leaf, whole),
                None => self.body_of(zones.state, leaf, rect),
            };
            let hosted = zones
                .state
                .active_entry(leaf)
                .and_then(Entry::group)
                .filter(|group| self.pane_rect(Tree::Group(*group)).is_some());
            if let Some(group) = hosted {
                if split {
                    for side in SIDES {
                        let edge = band(body, side, OUTER_EDGE);
                        self.zone(zones, DockDrop::Split { leaf, side }, edge, Some(rect));
                    }
                }
                self.zones_in(zones, Tree::Group(group), true, None);
            }
            if split {
                for side in SIDES {
                    let reach = match side.direction() {
                        Direction::Horizontal => rect.width(),
                        Direction::Vertical => rect.height(),
                    } * EDGE_ZONE;
                    let edge = band(rect, side, reach).intersect(body);
                    self.zone(zones, DockDrop::Split { leaf, side }, edge, Some(rect));
                }
            }
            self.zone(zones, middle_target(zones.state, leaf), body, None);
        }
    }

    /// A pane without its tab bar, which is the bar's to part for a carried tab.
    fn body_of(&self, state: &DockState, leaf: LeafId, rect: Rect) -> Rect {
        let Some(bar) = self.bar_rect(leaf) else {
            return rect;
        };
        let mut body = rect;
        match state.is_vertical(leaf) {
            true if bar.right() > body.left() && bar.right() < body.right() => {
                body.min.x = bar.right();
            }
            true => {}
            false => body.min.y = body.min.y.max(bar.bottom()).min(body.max.y),
        }
        body
    }

    /// A place chosen in `zone`; a split's only where the pointer is nearer its side of
    /// `pane` than any other, in proportion, so each corner splits along its diagonal.
    fn zone(&self, zones: &mut Zones, drop: DockDrop, zone: Rect, pane: Option<Rect>) {
        if zone.width() <= 0.0 || zone.height() <= 0.0 {
            return;
        }
        let Some(place) = self.place(zones.state, zones.dragged, drop, Some(zones.surface)) else {
            return;
        };
        let rest = place.rest.unwrap_or(zone);
        let label = self.place_label(zones.state, drop);
        let local = -zones.origin.to_vec2();
        let mut candidate = Candidate::new(place, rest.translate(local), zones.node)
            .zoned(zone.translate(local))
            .labelled(label);
        if let (DockDrop::Split { side, .. }, Some(pane)) = (drop, pane) {
            candidate = candidate.nearest_edge(side, pane.translate(local));
        }
        zones.out.push(candidate);
    }

    /// What a screen reader says for a place: "left of Files", "with Files".
    fn place_label(&self, state: &DockState, drop: DockDrop) -> String {
        let named = |leaf: LeafId| {
            state
                .active_entry(leaf)
                .map(|entry| self.entry_title(entry))
                .unwrap_or_else(|| "the empty pane".to_owned())
        };
        match drop {
            DockDrop::Split { leaf, side } => {
                let side = match side {
                    Side::Left => "left of",
                    Side::Right => "right of",
                    Side::Above => "above",
                    Side::Below => "below",
                };
                format!("{side} {}", named(leaf))
            }
            DockDrop::Group { leaf, .. } => format!("with {}", named(leaf)),
            DockDrop::Tab { leaf, .. } => format!("in {}", named(leaf)),
            DockDrop::Window { .. } => "in a window of its own".to_owned(),
        }
    }

    /// The place `drop` is for `dragged`, if the state admits it there and it changes
    /// anything: the state after it and where the item rests in it.
    pub(super) fn place(
        &self,
        state: &DockState,
        dragged: DockDragged,
        drop: DockDrop,
        surface: Option<SurfaceId>,
    ) -> Option<DockPlace> {
        let admitted = match dragged {
            DockDragged::Entry(entry) => {
                !drop
                    .leaf()
                    .is_some_and(|leaf| state.contains_leaf(entry, leaf))
                    && state.admits(entry, drop)
            }
            DockDragged::Pane(leaf) => state.admits_leaf(leaf, drop),
            DockDragged::Bar(_) => false,
        };
        if !admitted {
            return None;
        }
        let mut after = state.clone();
        let moved = match dragged {
            DockDragged::Entry(entry) => {
                after.drop_entry(entry, drop);
                entry
            }
            DockDragged::Pane(leaf) => {
                let shown = state.active_entry(leaf);
                after.drop_leaf(leaf, drop);
                shown?
            }
            DockDragged::Bar(_) => return None,
        };
        if after == *state {
            return None;
        }
        let rest = after
            .locate(moved)
            .and_then(|(leaf, _)| self.rect_after(&after, leaf, 0));
        Some(DockPlace {
            drop,
            after: Rc::new(after),
            surface,
            rest,
        })
    }

    /// Where `leaf` is laid out in `after`, in its window's points: in the area its tree is
    /// drawn in now, or, for a group the drop makes, in the pane that will host it.
    fn rect_after(&self, after: &DockState, leaf: LeafId, depth: usize) -> Option<Rect> {
        let tree = after.tree_of(leaf)?;
        let area = match tree {
            Tree::Surface(surface) => self.pane_rect(tree).or_else(|| {
                let rect = after.window_rect(surface)?;
                Some(rect.translate(self.rect.get_untracked().min.to_vec2()))
            })?,
            Tree::Group(group) => match self.pane_rect(tree) {
                Some(area) => area,
                None if depth < 8 => {
                    let (host, _) = after.locate(Entry::Group(group))?;
                    self.rect_after(after, host, depth + 1)?
                }
                None => return None,
            },
        };
        layout_tree(after, tree, area, self.thickness).leaf_rect(leaf)
    }

    /// Whether `point` reaches the places of `surface`, laid out relative to `node`: it is
    /// the topmost of the dock's surfaces holding the point (`topmost`).
    fn reaches(&self, node: NodeRef, surface: SurfaceId, point: DragPoint) -> bool {
        let Some(id) = node.try_get() else {
            return false;
        };
        let Some((placed, scale)) = beui_view::drag::space().placed(id) else {
            return false;
        };
        let local = Pos2::ZERO + (point.pos - placed.min) / scale.max(f32::EPSILON);
        self.open_at(node, local)
            .is_some_and(|(window, pos)| self.topmost(window, pos) == Some(surface))
    }

    /// Of the dock's surfaces in `window` (its main tiles and its windows), the topmost
    /// holding `pos`, if nothing but the dock's own layers covers it there. A dock dialog
    /// covers nothing: a tab carried over its own window's Save-as reaches what is under it.
    fn topmost(&self, window: WindowId, pos: Pos2) -> Option<SurfaceId> {
        let state = self.state.get_untracked();
        let surfaces: Vec<(NodeId, SurfaceId)> = state
            .surfaces()
            .into_iter()
            .filter_map(|surface| {
                let window = self.windows.value().borrow().get(&surface).copied();
                let node = window.or_else(|| {
                    self.panes
                        .value()
                        .borrow()
                        .get(&Tree::Surface(surface))
                        .copied()
                })?;
                Some((node.try_get()?, surface))
            })
            .collect();
        let ours: Vec<NodeId> = surfaces
            .iter()
            .map(|(node, _)| *node)
            .chain(
                self.dialog_layers
                    .value()
                    .borrow()
                    .values()
                    .filter_map(NodeRef::try_get),
            )
            .collect();
        stacked_at(&ours, window, pos)
            .into_iter()
            .find_map(|stacked| {
                let (_, surface) = surfaces.iter().find(|(node, _)| *node == stacked)?;
                measure(stacked)
                    .get_untracked()
                    .is_some_and(|measured| measured.rect.contains(pos))
                    .then_some(*surface)
            })
    }

    /// `node` and the dock's windows and dialogs: the layers a place in `node` may be under
    /// and still be reached.
    fn ours(&self, node: NodeId) -> Vec<NodeId> {
        std::iter::once(node)
            .chain(
                self.windows
                    .value()
                    .borrow()
                    .values()
                    .chain(self.dialog_layers.value().borrow().values())
                    .filter_map(NodeRef::try_get),
            )
            .collect()
    }

    /// The window and the point in its layout of `local`, a point in `node`, if no layer
    /// but the dock's own windows and dialogs covers it there and it is over `node` or one of
    /// those.
    fn open_at(&self, node: NodeRef, local: Pos2) -> Option<(WindowId, Pos2)> {
        let node = node.try_get()?;
        let rect = measure(node).get_untracked()?.rect;
        let pos = rect.min + local.to_vec2();
        let window = window_of(node);
        // `ours` may hold nodes of host-opened windows too: `stacked_at` keeps only those
        // in `window`.
        let stacked = stacked_at(&self.ours(node), window, pos);
        // A floating window may reach past the tiles: a point over it counts too.
        let over = |node: NodeId| {
            measure(node)
                .get_untracked()
                .is_some_and(|measured| measured.rect.contains(pos))
        };
        let open =
            !stacked.is_empty() && (rect.contains(pos) || stacked.iter().any(|node| over(*node)));
        open.then_some((window, pos))
    }

    /// The state the dock is drawn as while a carry of its own is over a place: the state
    /// after the drop there; tracked.
    pub(super) fn previewed(&self) -> Option<Rc<DockState>> {
        self.carried()?;
        self.over
            .get()
            .and_then(|over| over.get())
            .or_else(|| self.bar_place.get())
            .map(|place| place.after)
    }

    /// Parts the dock too while `leaf`'s tab bar parts for a carried tab or pane from
    /// elsewhere, as dropping it there would leave the dock (its old pane closing).
    pub(super) fn part_for_bar(&self, leaf: LeafId, gap: Memo<Option<ReorderGap<Entry>>>) {
        let dock = *self;
        let before = create_memo(move || gap.with(|gap| gap.as_ref().map(|gap| gap.before)));
        create_effect(move || {
            let place = before.get().and_then(|before| {
                let dragged = dock.carried()?;
                untrack(|| {
                    let state = dock.state.get_untracked();
                    let entries = state.entries(leaf);
                    let index = before
                        .and_then(|before| entries.iter().position(|entry| *entry == before))
                        .unwrap_or(entries.len());
                    let surface = state.surface_of(leaf);
                    dock.place(&state, dragged, DockDrop::Tab { leaf, index }, surface)
                })
            });
            let ours = |held: &Option<DockPlace>| {
                held.as_ref().and_then(|held| held.drop.leaf()) == Some(leaf)
            };
            match place {
                Some(place) => dock.bar_place.set(Some(place)),
                None if dock.bar_place.with_untracked(ours) => dock.bar_place.set(None),
                None => {}
            }
        });
        on_cleanup(move || {
            if dock.bar_place.is_alive()
                && dock
                    .bar_place
                    .with_untracked(|held| held.as_ref().and_then(|held| held.drop.leaf()))
                    == Some(leaf)
            {
                dock.bar_place.set(None);
            }
        });
    }

    /// Drops `dragged` on `place`, and glides it there from where it was drawn.
    pub(super) fn commit_drop(&self, dragged: DockDragged, place: DockPlace) {
        let drawn = self.ghost_at.get();
        self.ghost_at.set(None);
        if let (Some(surface), Some(to), Some((at, from))) = (place.surface, place.rest, drawn)
            && at == surface
        {
            self.landing.set(Some(Landing {
                surface,
                dragged,
                from,
                to,
            }));
        }
        let moving = match dragged {
            DockDragged::Entry(entry) => self.focus_moving_with(entry),
            _ => None,
        };
        self.edit(|state| {
            match dragged {
                DockDragged::Entry(entry) => state.drop_entry(entry, place.drop),
                DockDragged::Pane(leaf) => state.drop_leaf(leaf, place.drop),
                DockDragged::Bar(_) => {}
            }
            self.keyboard_follows_move(state, moving);
        });
    }

    /// The tab a group drop would group the carried item with, unless tidying would only
    /// fold it into the pane.
    fn groups_with(&self, dragged: DockDragged, place: &DockPlace) -> Option<String> {
        let DockDrop::Group { leaf, index } = place.drop else {
            return None;
        };
        let state = self.state.get_untracked();
        let onto = state.entries(leaf).get(index).copied()?;
        let moved = match dragged {
            DockDragged::Entry(entry) => entry,
            DockDragged::Pane(pane) => state.active_entry(pane)?,
            DockDragged::Bar(_) => return None,
        };
        let (landed, _) = place.after.locate(moved)?;
        (landed != leaf).then(|| self.entry_title(onto))
    }
}

const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Above, Side::Below];

/// What gathering a surface's places carries along.
struct Zones<'a> {
    state: &'a DockState,
    dragged: DockDragged,
    surface: SurfaceId,
    origin: Pos2,
    node: NodeRef,
    out: &'a mut Vec<Candidate<DockPlace>>,
}

/// The band `reach` deep along `rect`'s `side`.
fn band(rect: Rect, side: Side, reach: f32) -> Rect {
    let mut band = rect;
    match side {
        Side::Left => band.max.x = (rect.min.x + reach).min(rect.max.x),
        Side::Right => band.min.x = (rect.max.x - reach).max(rect.min.x),
        Side::Above => band.max.y = (rect.min.y + reach).min(rect.max.y),
        Side::Below => band.min.y = (rect.max.y - reach).max(rect.min.y),
    }
    band
}

/// How far each side of a pane's rect is drawn from where its layout puts it, and how fast
/// that changes, as a spring pulls it home.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub(super) struct Moving {
    min: Vec2,
    max: Vec2,
    min_speed: Vec2,
    max_speed: Vec2,
}

impl Moving {
    fn step(&mut self, spring: Spring, seconds: f32) -> bool {
        spring.advance(&mut self.min.x, &mut self.min_speed.x, seconds);
        spring.advance(&mut self.min.y, &mut self.min_speed.y, seconds);
        spring.advance(&mut self.max.x, &mut self.max_speed.x, seconds);
        spring.advance(&mut self.max.y, &mut self.max_speed.y, seconds);
        *self != Moving::default()
    }

    fn apply(&self, rect: Rect) -> Rect {
        Rect::from_min_max(rect.min + self.min, rect.max + self.max)
    }
}

/// A tree canvas's panes and splitters springing from where they are drawn to where the
/// layout it shows puts them: the real one, or the one a carried item's place would leave.
#[derive(Clone, Copy)]
pub(super) struct Parting {
    moving: Signal<Rc<HashMap<u64, Moving>>>,
    /// What the last layout drew, and the area it drew in.
    drawn: Stored<std::cell::RefCell<(Rect, HashMap<u64, Rect>)>>,
    frames: Animation,
}

impl Parting {
    pub(super) fn new(spring: Spring) -> Self {
        let moving = create_signal(Rc::new(HashMap::<u64, Moving>::default()));
        let frames = create_animation(
            move |frame| {
                let seconds = frame.delta.as_secs_f32();
                let mut next = (*moving.get_untracked()).clone();
                next.retain(|_, moved| moved.step(spring, seconds));
                let more = !next.is_empty();
                moving.set(Rc::new(next));
                more
            },
            move || {
                if moving.is_alive() {
                    moving.set(Rc::default());
                }
            },
        );
        Self {
            moving,
            drawn: store(std::cell::RefCell::new((Rect::ZERO, HashMap::default()))),
            frames,
        }
    }

    /// The springs now; tracked, so the arrangement over them follows each frame.
    pub(super) fn moving(&self) -> Rc<HashMap<u64, Moving>> {
        self.moving.get()
    }

    /// The layout changed to `target` (or back to `shape`'s): each item springs from where
    /// it was drawn. A pane that only exists after the drop grows out of its edge.
    pub(super) fn part(
        &self,
        shape: Option<&DockShape>,
        target: Option<&DockShape>,
        thickness: f32,
        reach: f32,
    ) {
        let Some(shape) = shape else {
            return;
        };
        let (area, drawn) = self.drawn.with(|drawn| drawn.borrow().clone());
        if area.width() <= 0.0 && area.height() <= 0.0 {
            return;
        }
        let laid = targets(shape, target, area, (thickness, reach));
        let grown = target.map(|target| edges(target, &|leaf| shape.leaves().contains(&leaf)));
        let mut next = HashMap::<u64, Moving>::default();
        for (key, rect) in laid {
            let from = drawn.get(&key).copied().or_else(|| {
                grown
                    .as_ref()?
                    .get(&key)
                    .map(|(direction, first)| collapsed(rect, *direction, *first))
            });
            let Some(from) = from else {
                continue;
            };
            let speed = self
                .moving
                .with_untracked(|moving| moving.get(&key).copied())
                .unwrap_or_default();
            let (min, max) = (from.min - rect.min, from.max - rect.max);
            // An item already where the new layout puts it stays there; one still away
            // keeps the speed it had, so a change of place mid-flight bends its path.
            if min == Vec2::ZERO && max == Vec2::ZERO {
                continue;
            }
            next.insert(key, Moving { min, max, ..speed });
        }
        self.moving.set(Rc::new(next));
        self.frames.start();
    }

    /// Where the items are drawn in `area`: where `target` (else `shape`) lays them out,
    /// displaced by their springs; what is drawn is kept for the next change.
    pub(super) fn arrange(
        &self,
        shape: &DockShape,
        target: Option<&DockShape>,
        moving: &HashMap<u64, Moving>,
        area: Rect,
        (thickness, reach): (f32, f32),
    ) -> Vec<(u64, Rect)> {
        let drawn: Vec<(u64, Rect)> = targets(shape, target, area, (thickness, reach))
            .into_iter()
            .map(|(key, rect)| match moving.get(&key) {
                Some(moved) => (key, moved.apply(rect)),
                None => (key, rect),
            })
            .collect();
        self.drawn.with(|held| {
            *held.borrow_mut() = (area, drawn.iter().copied().collect());
        });
        drawn
    }
}

/// Where each pane and splitter of `shape` lies in `area` once laid out as `target`: a pane
/// that would close shrinks into its edge away from the neighbour that takes its room, a
/// splitter that would go goes with it, and a pane only `target` has is laid out too, though
/// nothing draws it until the drop makes it. One pass over each shape.
fn targets(
    shape: &DockShape,
    target: Option<&DockShape>,
    area: Rect,
    (thickness, reach): (f32, f32),
) -> Vec<(u64, Rect)> {
    let Some(target) = target else {
        return arranged(shape, area, thickness, reach);
    };
    let mut out = arranged(target, area, thickness, reach);
    let shown: HashMap<u64, ()> = out.iter().map(|(key, _)| (*key, ())).collect();
    let here = shape.layout(area, thickness);
    let gone = edges(shape, &|leaf| !shown.contains_key(&leaf_key(leaf)));
    for (leaf, rect) in here.leaves {
        let key = leaf_key(leaf);
        if shown.contains_key(&key) {
            continue;
        }
        let shrunk = match gone.get(&key) {
            Some((direction, first)) => collapsed(rect, *direction, *first),
            None => Rect::from_center_size(rect.center(), Vec2::ZERO),
        };
        out.push((key, shrunk));
    }
    for splitter in here.splitters {
        let key = split_key(splitter.id);
        if shown.contains_key(&key) {
            continue;
        }
        out.push((
            key,
            Rect::from_center_size(splitter.handle.center(), Vec2::ZERO),
        ));
    }
    out
}

/// Of `shape`'s panes that `pick` picks and that sit directly in a split, which way that
/// split runs and whether the pane is its first: the edge it shrinks into or grows out of,
/// away from its neighbour.
fn edges(shape: &DockShape, pick: &dyn Fn(LeafId) -> bool) -> HashMap<u64, (Direction, bool)> {
    fn walk(
        shape: &DockShape,
        pick: &dyn Fn(LeafId) -> bool,
        out: &mut HashMap<u64, (Direction, bool)>,
    ) {
        let DockShape::Split {
            direction,
            first,
            second,
            ..
        } = shape
        else {
            return;
        };
        for (child, is_first) in [(first, true), (second, false)] {
            match child.as_ref() {
                DockShape::Leaf(leaf) if pick(*leaf) => {
                    out.insert(leaf_key(*leaf), (*direction, is_first));
                }
                DockShape::Leaf(_) => {}
                split => walk(split, pick, out),
            }
        }
    }
    let mut out = HashMap::default();
    walk(shape, pick, &mut out);
    out
}

/// `rect` shrunk to nothing along `direction` at its outer edge: the start for the first
/// child of a split, the end for the second.
fn collapsed(rect: Rect, direction: Direction, first: bool) -> Rect {
    let mut shrunk = rect;
    match (direction, first) {
        (Direction::Horizontal, true) => shrunk.max.x = rect.min.x,
        (Direction::Horizontal, false) => shrunk.min.x = rect.max.x,
        (Direction::Vertical, true) => shrunk.max.y = rect.min.y,
        (Direction::Vertical, false) => shrunk.min.y = rect.max.y,
    }
    shrunk
}

/// Lays a pane's panel out at the size it had when a carry of the dock was lifted, until the
/// carry ends, clipped by the pane as it moves: resizing a panel each frame would lay out and
/// present every app and program in it again (guides/apps/runtime.md, Settling).
#[component]
pub(super) fn DockHeldSize(dock: State, children: Child) -> NodeId {
    let held = create_signal(None::<Vec2>);
    // Measured from the start, so the lift reads the size at once, also where measurement
    // comes a frame late.
    let measured = create_signal(None::<ReadSignal<Option<Measured>>>);
    // A carried tool bar only opens and closes bands, so the panels beside them resize then.
    let carrying = create_memo(move || dock.carried().is_some());
    create_effect_on(
        move || carrying.get(),
        move |&carrying, _| {
            let size = carrying
                .then(|| measured.get_untracked())
                .flatten()
                .and_then(|measured| measured.get_untracked())
                .map(|measured| measured.rect.size());
            held.set(size);
        },
    );
    let arrangement = create_memo(move || {
        let held = held.get();
        Some(CanvasArrangement::new(move |size| {
            vec![(0, Rect::from_min_size(Pos2::ZERO, held.unwrap_or(size)))]
        }))
    });
    let canvas = view! {
        <Canvas arrangement>
            <CanvasItem arranged=0u64 clip=false>{children}</CanvasItem>
        </Canvas>
    };
    measured.set(Some(measure(canvas)));
    canvas
}

/// The carried tab or pane over one of `surface`'s places, drawn the size of where it
/// would rest and held under the pointer, and once dropped there, gliding into that rest.
/// `origin` is where the surface's canvas is, in its window's points.
#[component]
pub(super) fn DockGhost(
    dock: State,
    surface: SurfaceId,
    origin: Memo<Pos2>,
) -> DynamicSegment<CanvasItem> {
    let board = drag_board();
    let carried = create_memo(move || {
        let dragged = dock.carried()?;
        let place = dock.over.get()?.get()?;
        if place.surface != Some(surface) {
            return None;
        }
        let frame = board
            .frame_of(&dock.carry(dragged))
            .filter(|frame| !frame.floating)?;
        Some((dragged, frame.item, place))
    });
    // Where it was drawn last, for the glide on release; forgotten once the carry ends, so
    // a later drop that drew no ghost glides from nowhere stale.
    create_effect(move || match carried.get() {
        Some((_, item, _)) => dock.ghost_at.set(Some((surface, item))),
        None if dock.carried().is_none()
            && dock
                .ghost_at
                .get()
                .is_some_and(|(drawn, _)| drawn == surface) =>
        {
            dock.ghost_at.set(None)
        }
        None => {}
    });
    // A drop glides from where the ghost was drawn into its rest.
    let glide = create_signal(Moving::default());
    let spring = dock.curves.settle;
    let frames = create_animation(
        move |frame| {
            let mut moved = glide.get_untracked();
            let more = moved.step(spring, frame.delta.as_secs_f32());
            glide.set(moved);
            more
        },
        move || {
            if glide.is_alive() {
                glide.set(Moving::default());
            }
            if dock.landing.is_alive()
                && dock.landing.with_untracked(|landing| {
                    landing.is_some_and(|landing| landing.surface == surface)
                })
            {
                dock.landing.set(None);
            }
        },
    );
    create_effect(move || {
        let Some(landing) = dock
            .landing
            .get()
            .filter(|landing| landing.surface == surface)
        else {
            return;
        };
        untrack(|| {
            glide.set(Moving {
                min: landing.from.min - landing.to.min,
                max: landing.from.max - landing.to.max,
                ..Moving::default()
            });
            frames.stop();
            frames.start();
        });
    });
    let shown = create_memo(move || {
        if let Some((dragged, item, place)) = carried.get() {
            return Some((dragged, item, Some(place)));
        }
        let landing = dock
            .landing
            .get()
            .filter(|landing| landing.surface == surface)?;
        Some((landing.dragged, glide.get().apply(landing.to), None))
    });
    let rect = create_memo(move || {
        shown
            .with(|shown| shown.as_ref().map(|(_, item, _)| *item))
            .map_or(Rect::ZERO, |item| item.translate(-origin.get().to_vec2()))
    });
    let dragged =
        create_memo(move || shown.with(|shown| shown.as_ref().map(|(dragged, ..)| *dragged)));
    let group_with = create_memo(move || {
        shown.with(|shown| {
            let (dragged, _, place) = shown.as_ref()?;
            dock.groups_with(*dragged, place.as_ref()?)
        })
    });
    let x = create_memo(move || rect.get().min.x);
    let y = create_memo(move || rect.get().min.y);
    let width = create_memo(move || rect.get().width());
    let height = create_memo(move || rect.get().height());
    let preview = dock.preview;
    view! {
        <Show value={move || dragged.get()}>|dragged: ReadSignal<DockDragged>| {
            let dragged = dragged.get_untracked();
            view! {
                <CanvasItem x={x} y={y} width={width} height={height} pointer=false>
                    {preview.call(DockPreviewHandle {
                        dragged,
                        title: create_memo(move || dock.dragged_title(dragged)),
                        icon: create_memo(move || dock.dragged_icon(dragged)),
                        opening: true,
                        group_with,
                    })}
                </CanvasItem>
            }
        }</Show>
    }
}
