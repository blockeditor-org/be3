//! A list whose rows are carried to new places in it, or into another `Reorder` that accepts
//! them: a drop site over `slots_in` whose rows are drag sources. While a row is carried the
//! list shows it at the place chosen and the rows between glide aside; a carry from another
//! list opens a gap where it would land. Only the release writes anything: `on_move` hears
//! the payload and where it goes, and the app makes the move
//! (decisions/beui_a_move_between_drop_sites_is_the_apps.md).

use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use be_lib_stable_hash::HashMap;

use crate::drag_board::drag_board;
use crate::drop_site::{carrying, slots_in, use_drop_site};
use accesskit::{Live, Node, Role};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::drag::{
    After, Carry, DragPoint, DragSpec, closest, fixed, floating, into, move_after,
};
use beui_view::primitives::*;
use beui_view::reactive::*;
use beui_view::reactive::{KeyAt, NodeId, OverlayAnchor};
use beui_view::types::{
    CursorIcon, Direction, Key, KeyPhase, LayerLevel, LayerPolicy, Placement, PointerPress, Pos2,
    Rect, Sides, TouchDrags,
};

use crate::glide::{Glide, GlideApi, GlideHold};

#[derive(Clone, Debug, PartialEq)]
pub struct ReorderMove<K, P> {
    pub payload: P,
    pub key: Option<K>,
    pub after: Option<K>,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReorderGap<K> {
    pub before: Option<K>,
    pub size: f32,
}

#[derive(Clone, Copy)]
pub struct ReorderItemHandle {
    /// Whether this row is carried: it stays in the list, at the place chosen, and the
    /// `preview` floats while it is over none.
    pub lifted: Memo<bool>,
    pub raised: Memo<bool>,
    pub draggable: Memo<bool>,
}

enum Arrival<K> {
    New(Vec<K>, Pos2),
}

struct State<K: 'static, P: 'static> {
    spring: Spring,
    keys: Memo<Vec<K>>,
    direction: Prop<Direction>,
    payload: Func<K, Option<P>>,
    disabled: Prop<bool>,
    fixed: Prop<bool>,
    slot: Option<Func<P, Option<usize>>>,
    lifts_on_long_press: bool,
    text: Callback<K, String>,
    on_move: Callback<ReorderMove<K, P>>,
    on_drag_change: Callback<Option<K>>,
    /// The row of this list that is carried.
    carried: Signal<Option<K>>,
    /// Whether the carry is on a place in this list.
    over_here: Signal<bool>,
    /// This list's row carried off it, collapsed while its slot closes, or while the slot
    /// opens again for it to come back.
    folded: Signal<Option<K>>,
    /// Whether the folded row's slot is opening for it to come back.
    returning: Cell<bool>,
    /// The folded row's slot as it was laid out before it collapsed: its length and spacing.
    folded_length: Cell<f32>,
    gap: Signal<Option<ReorderGap<K>>>,
    aim: Cell<f32>,
    speed: Cell<f32>,
    frames: Cell<Option<Animation>>,
    visited: Signal<bool>,
    raised: Signal<Option<K>>,
    announcement: Signal<String>,
    arrival: RefCell<Option<Arrival<K>>>,
    /// Each shown row's glide, by the glide's node: its inner glide's node and its api.
    glides: RefCell<HashMap<NodeId, (NodeId, GlideApi)>>,
    /// The rows shown, as the `ForEach` or `VirtualList` that shows them reports them.
    rows: ForEachRef<K, NodeId>,
    /// Counts the moves the `Reorder` makes (a carry to another place, a drop, a step): rows
    /// glide only for an order or gap changed by one, so an order changed for any other
    /// reason snaps, as any list does.
    moves: Signal<u64>,
    /// The node the rows sit in, which their places are relative to.
    area: NodeRef,
}

pub struct ReorderHandle<K: 'static, P: 'static> {
    state: Stored<State<K, P>>,
    /// Each key's place in `order`.
    places: Memo<Rc<HashMap<K, usize>>>,
    pub order: Memo<Vec<K>>,
    /// Hand this to the `ForEach` or `VirtualList` that shows `order`, as its `handle`: it is
    /// how the list finds its rows.
    pub rows: ForEachRef<K, NodeId>,
    pub dragging: Memo<Option<K>>,
    pub gap: Memo<Option<ReorderGap<K>>>,
    pub visited: Memo<bool>,
    pub direction: Prop<Direction>,
}

impl<K, P> Clone for ReorderHandle<K, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, P> Copy for ReorderHandle<K, P> {}

impl<K, P> ReorderHandle<K, P>
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    pub fn step(&self, key: K, by: isize) -> bool {
        step(self.state, key, by)
    }

    /// Opens a gap where an item at `point` would land, or closes it with `None`: for a
    /// caller that tracks a drag of its own over the list (a tree dropping into a folder).
    pub fn hover(&self, point: Option<DragPoint>) {
        visit(self.state, point);
    }

    pub(crate) fn part(&self, gap: Option<ReorderGap<K>>) {
        open_gap(self.state, gap.is_some(), gap);
    }

    pub(crate) fn shut(&self) {
        self.state.with(|state| {
            state.visited.set(false);
            shut(state);
        });
    }

    pub fn draggable(&self, key: K) -> bool {
        self.state.with(|state| can_drag(state, &key))
    }

    pub fn resting(&self, key: &K) -> Option<Rect> {
        self.state.with(|state| {
            let (_, _, inner, glide) = glide_of(state, key)?;
            resting(glide, inner)
        })
    }
}

fn along(direction: Direction, pos: Pos2) -> f32 {
    direction.main(pos.to_vec2())
}

fn can_drag<K: Clone, P>(state: &State<K, P>, key: &K) -> bool {
    !state.disabled.get_untracked() && state.payload.call(key.clone()).is_some()
}

/// Marks a move the `Reorder` makes, so the rows it moves glide.
fn mark_move<K, P>(state: &State<K, P>) {
    state.moves.update(|moves| *moves += 1);
}

/// Each shown row's key, glide node, inner glide node and glide api, in the order shown.
/// Walks the rows the list built, not every key it has.
fn shown_glides<K: Clone + PartialEq, P>(
    state: &State<K, P>,
) -> Vec<(K, NodeId, NodeId, GlideApi)> {
    let glides = state.glides.borrow();
    let mut shown = Vec::new();
    state.rows.for_each_row(|row| {
        for node in &row.nodes {
            if let Some((inner, glide)) = glides.get(node) {
                shown.push((row.key.clone(), *node, *inner, *glide));
            }
        }
    });
    shown
}

fn glide_of<K: Clone + PartialEq, P>(
    state: &State<K, P>,
    key: &K,
) -> Option<(K, NodeId, NodeId, GlideApi)> {
    let glides = state.glides.borrow();
    state.rows.nodes_of(key).into_iter().find_map(|node| {
        let (inner, glide) = glides.get(&node)?;
        Some((key.clone(), node, *inner, *glide))
    })
}

fn resting(glide: GlideApi, inner: NodeId) -> Option<Rect> {
    let rect = measure(inner).get_untracked()?.rect;
    Some(rect.translate(-glide.displacement()))
}

fn spans<K: Clone + PartialEq, P>(state: &State<K, P>) -> Vec<(f32, f32)> {
    let direction = state.direction.get_untracked();
    let mut spans: Vec<(f32, f32)> = shown_glides(state)
        .iter()
        .filter_map(|(_, outer, ..)| {
            measure(*outer)
                .get_untracked()
                .map(|measured| measured.rect)
        })
        .map(|rect| (along(direction, rect.min), along(direction, rect.max)))
        .collect();
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    spans
}

/// Where a row held at `item` is drawn: along the list no further than its ends, as a row
/// that has nowhere else to go never leaves its list.
fn clamped<K, P>(state: &State<K, P>, item: Rect) -> Pos2 {
    let Some(area) = state
        .area
        .try_get()
        .and_then(|area| measure(area).get_untracked())
        .map(|measured| measured.rect)
    else {
        return item.min;
    };
    let direction = state.direction.get_untracked();
    let low = along(direction, area.min);
    let high = (along(direction, area.max) - direction.main(item.size())).max(low);
    match direction {
        Direction::Horizontal => Pos2::new(item.min.x.clamp(low, high), item.min.y),
        Direction::Vertical => Pos2::new(item.min.x, item.min.y.clamp(low, high)),
    }
}

/// The space between two rows, the least of any pair shown.
fn spacing<K: Clone + PartialEq, P>(state: &State<K, P>) -> f32 {
    spans(state)
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].1)
        .filter(|between| *between >= 0.0)
        .reduce(f32::min)
        .unwrap_or(0.0)
}

/// How far the rows part for an item of `item`'s size: its length and the spacing.
fn gap_for<K: Clone + PartialEq, P>(state: &State<K, P>, item: Rect) -> f32 {
    state.direction.get_untracked().main(item.size()) + spacing(state)
}

/// The index among `keys` an item spanning `span` along the list lands at: before the first
/// row whose middle it has not passed.
fn slot_at<K: Clone + Eq + Hash, P>(state: &State<K, P>, keys: &[K], span: (f32, f32)) -> usize {
    let direction = state.direction.get_untracked();
    let places: HashMap<&K, usize> = keys.iter().enumerate().map(|(at, key)| (key, at)).collect();
    let mut middles: Vec<(f32, usize)> = shown_glides(state)
        .iter()
        .filter_map(|(key, _, inner, glide)| {
            let index = *places.get(key)?;
            let rect = resting(*glide, *inner)?;
            Some((along(direction, rect.center()), index))
        })
        .collect();
    middles.sort_by(|left, right| left.0.total_cmp(&right.0));
    match middles.iter().find(|(middle, _)| span.0 < *middle) {
        Some((_, index)) => *index,
        None => middles.last().map_or(keys.len(), |(_, index)| index + 1),
    }
}

fn announce<K: Clone, P>(state: &State<K, P>, key: &K, index: usize, count: usize) {
    let name = match state.text.is_empty() {
        true => "Item".to_owned(),
        false => state.text.call(key.clone()),
    };
    state
        .announcement
        .set(format!("{name} moved to position {} of {count}", index + 1));
}

/// A drag the `Reorder` does not carry, at `point` over the list or gone with `None`: the
/// rows part where it would land.
fn visit<K, P>(stored: Stored<State<K, P>>, point: Option<DragPoint>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let gap = stored.with(|state| {
        let point = point?;
        let keys = state.keys.get_untracked();
        let item = point.item.translate(point.pos.to_vec2());
        let direction = state.direction.get_untracked();
        let index = slot_at(
            state,
            &keys,
            (along(direction, item.min), along(direction, item.max)),
        );
        Some(ReorderGap {
            before: keys.get(index).cloned(),
            size: gap_for(state, point.item),
        })
    });
    open_gap(stored, point.is_some(), gap);
}

fn open_gap<K, P>(stored: Stored<State<K, P>>, visited: bool, gap: Option<ReorderGap<K>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let frames = stored.with(|state| {
        state.visited.set(visited);
        let aim = gap.as_ref().map_or(0.0, |gap| gap.size);
        match (gap, state.gap.get_untracked()) {
            (Some(gap), Some(shown)) if gap.before != shown.before => {
                mark_move(state);
                state.gap.set(Some(ReorderGap {
                    before: gap.before,
                    size: shown.size,
                }));
            }
            (Some(gap), None) => state.gap.set(Some(ReorderGap {
                before: gap.before,
                size: 0.0,
            })),
            _ => {}
        }
        state.aim.set(aim);
        let shown = state
            .gap
            .with_untracked(|shown| shown.as_ref().map_or(0.0, |shown| shown.size));
        (shown != aim).then(|| state.frames.get()).flatten()
    });
    if let Some(frames) = frames {
        frames.start();
    }
}

fn shut<K: Clone + PartialEq, P>(state: &State<K, P>) {
    state.aim.set(0.0);
    state.speed.set(0.0);
    if state.gap.with_untracked(Option::is_some) {
        state.gap.set(None);
    }
}

fn step_gap<K, P>(stored: Stored<State<K, P>>, seconds: f32) -> bool
where
    K: Clone + PartialEq + 'static,
    P: 'static,
{
    stored
        .try_with(|state| {
            let Some(mut shown) = state.gap.get_untracked() else {
                return false;
            };
            let aim = state.aim.get();
            let mut offset = shown.size - aim;
            let mut speed = state.speed.get();
            state.spring.advance(&mut offset, &mut speed, seconds);
            state.speed.set(speed);
            shown.size = (aim + offset).max(0.0);
            let moving = offset != 0.0 || speed != 0.0;
            state.gap.set((moving || aim > 0.0).then_some(shown));
            moving
        })
        .unwrap_or(false)
}

fn settle_gap<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + PartialEq + 'static,
    P: 'static,
{
    stored.try_with(|state| {
        state.speed.set(0.0);
        let aim = state.aim.get();
        if let Some(mut shown) = state.gap.get_untracked()
            && shown.size != aim
        {
            shown.size = aim;
            state.gap.set((aim > 0.0).then_some(shown));
        }
        // The slot is open again: the row it was opened for takes it.
        if state.returning.replace(false) {
            state.folded.set(None);
            shut(state);
        }
    });
}

/// This list's row `key` is carried off it (`away`) or comes back: it collapses and the slot
/// it leaves closes on the spring, or, let go where nothing takes it, the slot opens again
/// before it uncollapses into it; carried back onto the list, it is shown at once.
fn fold<K, P>(stored: Stored<State<K, P>>, key: K, away: bool, length: f32)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let frames = stored.with(|state| {
        let keys = state.keys.get_untracked();
        let at = keys.iter().position(|shown| *shown == key);
        let before = at.and_then(|at| keys.get(at + 1).cloned());
        // Measured as it leaves; collapsed, it measures nothing on its way back.
        if away {
            state.folded_length.set(length + spacing(state));
        }
        let size = state.folded_length.get();
        let carried = state
            .carried
            .with_untracked(|carried| carried.as_ref() == Some(&key));
        match (away, at.is_some() && !carried) {
            (true, _) => {
                state.returning.set(false);
                state.folded.set(Some(key));
                state.gap.set(Some(ReorderGap { before, size }));
                state.aim.set(0.0);
            }
            (false, true) => {
                state.returning.set(true);
                let shown = state
                    .gap
                    .with_untracked(|gap| gap.as_ref().map_or(0.0, |gap| gap.size));
                state.gap.set(Some(ReorderGap {
                    before,
                    size: shown,
                }));
                state.aim.set(size);
            }
            (false, false) => {
                state.returning.set(false);
                state.folded.set(None);
                shut(state);
                return None;
            }
        }
        state.speed.set(0.0);
        state.frames.get()
    });
    if let Some(frames) = frames {
        frames.start();
    }
}

/// The places a carried `payload` can land in this list: its own row's among the rows,
/// or, from elsewhere, one at each gap; a `fixed` list offers only where it sorts the item
/// (its own row's home, or the place `slot` names).
fn spec<K, P>(stored: Stored<State<K, P>>, payload: P) -> DragSpec<After<K>>
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    stored.with(|state| {
        let own = state.carried.get_untracked();
        let keys = state.keys.get_untracked();
        let slots = slots_in(
            state.area,
            state.rows,
            own.as_ref(),
            state.direction.get_untracked(),
        );
        let direction = state.direction.get_untracked();
        // A row of this list that no other site takes stays on this list's places however
        // far it is carried, as it has nowhere else to go; anything else floats when far.
        let far = match own.is_some() && drag_board().takers(&payload) <= 1 {
            true => None,
            false => Some(floating()),
        };
        let reach = move |spec: DragSpec<After<K>>| match far.clone() {
            Some(far) => spec.when_far(far),
            None => spec,
        };
        if !state.fixed.get_untracked() {
            // Sticky by the spacing between rows: a row trades places with its neighbour once
            // its leading edge passes the neighbour's middle, having covered half of it,
            // whatever their lengths.
            return reach(
                closest(slots)
                    .with_distance(spacing(state))
                    .following(direction),
            );
        }
        // A fixed list sorts its rows itself: its own row floats at once, its slot kept
        // open, and let go over this list it lands home.
        if let Some(own) = &own {
            let home = keys.iter().position(|key| key == own).unwrap_or(0);
            return into(
                state.area,
                After(home.checked_sub(1).map(|at| keys[at].clone())),
            );
        }
        let index = match &own {
            Some(_) => None,
            None => state
                .slot
                .and_then(|slot| slot.call(payload))
                .map(|index| index.min(keys.len())),
        };
        let after = index.map(|index| After(index.checked_sub(1).map(|at| keys[at].clone())));
        match slots
            .into_iter()
            .find(|slot| Some(&slot.drop) == after.as_ref())
        {
            Some(slot) => reach(fixed(slot).following(direction)),
            None => floating(),
        }
    })
}

/// A carried `payload` let go on the place after `after`: the app hears the move, and a row
/// from elsewhere glides in from where it was let go.
fn commit<K, P>(stored: Stored<State<K, P>>, payload: P, after: Option<K>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let moved = stored.with(|state| {
        let keys = state.keys.get_untracked();
        let own = state.carried.get_untracked();
        let rest: Vec<K> = keys
            .iter()
            .filter(|key| Some(*key) != own.as_ref())
            .cloned()
            .collect();
        let index = match &after {
            Some(after) => rest
                .iter()
                .position(|key| key == after)
                .map_or(0, |at| at + 1),
            None => 0,
        };
        let from = own
            .as_ref()
            .and_then(|own| keys.iter().position(|key| key == own));
        if own.is_none()
            && let Some(frame) = untrack(|| drag_board().frame())
        {
            *state.arrival.borrow_mut() = Some(Arrival::New(keys.clone(), frame.item.min));
        }
        shut(state);
        state.visited.set(false);
        mark_move(state);
        if let Some(own) = &own {
            announce(state, own, index, keys.len());
        }
        (from != Some(index)).then(|| ReorderMove {
            payload,
            key: own,
            after,
            index,
        })
    });
    if let Some(moved) = moved {
        stored.with(|state| state.on_move.call(moved));
    }
}

fn step<K, P>(stored: Stored<State<K, P>>, key: K, by: isize) -> bool
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let moved = stored.with(|state| {
        if !can_drag(state, &key)
            || state.fixed.get_untracked()
            || state.carried.with_untracked(Option::is_some)
        {
            return None;
        }
        let keys = state.keys.get_untracked();
        let from = keys.iter().position(|shown| *shown == key)?;
        let to = from.saturating_add_signed(by).min(keys.len() - 1);
        if to == from {
            return None;
        }
        let rest: Vec<K> = keys
            .iter()
            .filter(|shown| **shown != key)
            .cloned()
            .collect();
        mark_move(state);
        state.raised.set(Some(key.clone()));
        announce(state, &key, to, keys.len());
        Some(ReorderMove {
            payload: state.payload.call(key.clone())?,
            key: Some(key.clone()),
            after: to.checked_sub(1).and_then(|at| rest.get(at).cloned()),
            index: to,
        })
    });
    let Some(moved) = moved else {
        return false;
    };
    stored.with(|state| state.on_move.call(moved));
    true
}

/// The carry a row of this list lifts, if it can be carried.
fn lift<K, P>(stored: Stored<State<K, P>>, key: K) -> Option<Carry>
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    stored.with(|state| {
        if !can_drag(state, &key) {
            return None;
        }
        let payload = state.payload.call(key.clone())?;
        state.carried.set(Some(key.clone()));
        state.raised.set(Some(key.clone()));
        state.on_drag_change.call(Some(key.clone()));
        // The list draws the row's preview itself, at its root, while it floats: the row is
        // collapsed by then.
        Some(Carry::new(payload).drawn_by_site())
    })
}

#[component]
pub fn Reorder<K, P>(
    value: Prop<Vec<K>>,
    spring: Spring,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    payload: Func<K, Option<P>>,
    accepts: Option<Func<P, bool>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] fixed: Prop<bool>,
    slot: Option<Func<P, Option<usize>>>,
    /// Whether a finger's long press lifts a row; off, the long press is the row's menu, and
    /// a finger lifts a row by dragging it across the list.
    #[prop(default = false)]
    lifts_on_long_press: bool,
    text: Callback<K, String>,
    on_move: Callback<ReorderMove<K, P>>,
    on_drag_change: Callback<Option<K>>,
    preview: RenderFn<P, NodeId>,
    #[prop(children)] content: Render<ReorderHandle<K, P>, NodeId>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let keys = create_memo(move || value.get());
    let carried = create_signal(None::<K>);
    let over_here = create_signal(false);
    let gap = create_signal(None::<ReorderGap<K>>);
    let visited = create_signal(false);
    let announcement = create_signal(String::new());
    let area = NodeRef::new();
    let state = store(State {
        spring,
        keys,
        direction,
        payload,
        disabled,
        fixed,
        slot,
        lifts_on_long_press,
        text,
        on_move,
        on_drag_change,
        carried,
        over_here,
        folded: create_signal(None),
        returning: Cell::new(false),
        folded_length: Cell::new(0.0),
        gap,
        aim: Cell::new(0.0),
        speed: Cell::new(0.0),
        frames: Cell::new(None),
        visited,
        raised: create_signal(None),
        announcement,
        arrival: RefCell::new(None),
        glides: RefCell::new(HashMap::default()),
        rows: ForEachRef::new(),
        moves: create_signal(0),
        area,
    });
    let site = use_drop_site(
        Func::new(move |payload: P| {
            carried.with_untracked(Option::is_some)
                || (!disabled.get_untracked()
                    && accepts.is_some_and(|accepts| accepts.call(payload)))
        }),
        Func::new(move |payload: P| spec(state, payload)),
        Callback::new(move |(payload, After(after)): (P, After<K>)| {
            commit(state, payload, after);
        }),
    );
    // The carry ends when the board carries nothing: whatever it was dropped on has heard.
    create_effect(move || {
        if drag_board().carried().is_none() && carried.with_untracked(Option::is_some) {
            carried.set(None);
            state.with(mark_move);
            on_drag_change.call(None);
        }
    });
    // A carry from elsewhere parts the rows where it would land; this list's own row is
    // shown there instead.
    create_effect_on_change(
        move || site.over.get(),
        move |over, _| {
            let own = carried.with_untracked(Option::is_some);
            over_here.set(over.is_some());
            state.with(mark_move);
            if own {
                return;
            }
            let gap = over.clone().map(|After(after)| {
                state.with(|state| {
                    let keys = state.keys.get_untracked();
                    let index = after
                        .and_then(|after| keys.iter().position(|key| *key == after))
                        .map_or(0, |at| at + 1);
                    let item = drag_board().carried_item().unwrap_or(Rect::ZERO);
                    ReorderGap {
                        before: keys.get(index).cloned(),
                        size: gap_for(state, item),
                    }
                })
            });
            open_gap(state, gap.is_some(), gap);
        },
    );
    let order = create_memo(move || {
        let mut order = keys.get();
        if let (Some(own), Some(After(after))) = (carried.get(), site.over.get())
            && order.contains(&own)
        {
            move_after(&mut order, own, after);
        }
        order
    });
    let frames = create_animation(
        move |frame| step_gap(state, frame.delta.as_secs_f32()),
        move || settle_gap(state),
    );
    state.with(|state| state.frames.set(Some(frames)));
    // Each key's place in `order`, made once per order rather than by each row.
    let places = create_memo(move || {
        order.with(|order| {
            let mut places = HashMap::<K, usize>::default();
            for (at, key) in order.iter().enumerate() {
                places.entry(key.clone()).or_insert(at);
            }
            Rc::new(places)
        })
    });
    let handle = ReorderHandle {
        state,
        places,
        order,
        rows: state.with(|state| state.rows),
        dragging: create_memo(move || carried.get()),
        gap: create_memo(move || gap.get()),
        visited: create_memo(move || visited.get()),
        direction,
    };
    let face = content.call(handle);
    // A carry from elsewhere is drawn in the gap it opens, from this list's own `preview`;
    // this list's own row, floating off it, is drawn here too, under the pointer: the row
    // itself is collapsed then, and what a collapsed row holds is not shown.
    let floating = create_memo(move || {
        carried.with(Option::is_some) && drag_board().frame().is_some_and(|frame| frame.floating)
    });
    let visiting = create_memo(move || {
        (visited.get() || floating.get()) && carrying::<P>().is_some()
    });
    let anchor = create_memo(move || {
        OverlayAnchor::Point(
            drag_board()
                .frame()
                .map_or(Pos2::ZERO, |frame| frame.item.min),
        )
    });
    let shown = view! {
        <Layers @node_ref=&area>
            {face}
            <ReorderStatus announcement={announcement.read_only()} />
            <Overlay
                anchor={anchor}
                placement=Placement::At
                policy=LayerPolicy::Inert
                level=LayerLevel::Top
                traps_focus=false
                open={visiting}
            >
                <Dynamic
                    value={move || carrying::<P>()}
                    fallback={|| view! {
                        <Frame />
                    }}
                >|carried: P| {
                    preview.call(carried)
                }</Dynamic>
            </Overlay>
        </Layers>
    };
    // Measured from the start, so a held row is kept within the list from its first frame.
    measure(shown);
    shown
}

#[component]
fn ReorderStatus(announcement: ReadSignal<String>) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Status);
        node.set_live(Live::Polite);
        let said = announcement.get();
        if !said.is_empty() {
            node.set_label(said);
        }
        node
    }));
    view! {
        <Frame />
    }
}

#[component]
pub fn ReorderItem<K, P>(
    reorder: ReorderHandle<K, P>,
    item: Prop<K>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    on_click: Callback<()>,
    #[prop(children)] content: Render<ReorderItemHandle, NodeId>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let state = reorder.state;
    let direction = reorder.direction;
    let key = create_memo(move || item.get());
    let (carried, over_here, raised_key, gap) =
        state.with(|state| (state.carried, state.over_here, state.raised, state.gap));
    let mine =
        create_memo(move || carried.with(|carried| key.with(|key| carried.as_ref() == Some(key))));
    let fixed = state.with(|state| state.fixed);
    // Carried and on a place in this list: shown there, following the pointer along it; a
    // fixed list keeps the row's slot open instead, empty, while its preview floats.
    let lifted = create_memo(move || mine.get() && over_here.get());
    let held = create_memo(move || lifted.get() && !fixed.get());
    // Carried off this list (floating, or over another): the row folds away, and comes back
    // from where it was let go if it is not taken.
    let away = create_memo(move || mine.get() && !over_here.get());
    let raised =
        create_memo(move || raised_key.with(|raised| key.with(|key| raised.as_ref() == Some(key))));
    let draggable = create_memo(move || {
        state.with(|state| !state.disabled.get() && state.payload.call(key.get()).is_some())
    });
    let lead = create_memo(move || {
        gap.with(|gap| match gap {
            Some(ReorderGap {
                before: Some(before),
                size,
            }) if key.with(|key| key == before) => *size,
            _ => 0.0,
        })
    });
    let order = reorder.order;
    let places = reorder.places;
    // A row's glides arm when a move the `Reorder` makes moves its key in the order, or moves
    // the gap it holds open past it: rows a gap passes shift with no change of place. A row a
    // `VirtualList` hands another key has not moved: it shows another row now.
    let place = create_memo(move || places.with(|places| key.with(|key| places.get(key).copied())));
    let below_gap = create_memo(move || {
        gap.with(|gap| match gap {
            Some(ReorderGap {
                before: Some(before),
                ..
            }) => {
                let at = places.with(|places| places.get(before).copied());
                at.zip(place.get()).is_some_and(|(at, place)| at <= place)
            }
            _ => false,
        })
    });
    // The count of moves rides with the place, so a row arms only for a place changed in the
    // same flush as a move the `Reorder` made.
    let shifted = create_trigger();
    let moves = state.with(|state| state.moves);
    create_effect_on_change(
        move || (key.get(), place.get(), below_gap.get(), moves.get()),
        move |(key, place, gap, moved), (was_key, was_place, was_gap, was_moved)| {
            if moved != was_moved && key == was_key && (place != was_place || gap != was_gap) {
                shifted.notify();
            }
        },
    );
    let arm = Trackable::from(shifted);
    let folded = state.with(|state| state.folded);
    let collapsed =
        create_memo(move || folded.with(|folded| key.with(|key| folded.as_ref() == Some(key))));
    // A gap at the end opens after the last row shown, not a collapsed one.
    let trail = create_memo(move || {
        gap.with(|gap| match gap {
            Some(ReorderGap { before: None, size }) => {
                let last = folded.with(|folded| {
                    order.with(|order| {
                        order
                            .iter()
                            .rev()
                            .find(|shown| Some(*shown) != folded.as_ref())
                            .cloned()
                    })
                });
                match last == Some(key.get()) {
                    true => *size,
                    false => 0.0,
                }
            }
            _ => 0.0,
        })
    });
    let padding = create_memo(move || match direction.get() {
        Direction::Horizontal => Sides {
            left: lead.get(),
            right: trail.get(),
            ..Sides::ZERO
        },
        Direction::Vertical => Sides {
            top: lead.get(),
            bottom: trail.get(),
            ..Sides::ZERO
        },
    });
    // A row dropped in from elsewhere glides in from where it was let go.
    let from = create_signal(None::<Pos2>);
    create_effect(move || {
        let key = key.get();
        let arrived = state.with(|state| {
            let mut arrival = state.arrival.borrow_mut();
            let at = match arrival.as_ref() {
                Some(Arrival::New(known, at)) if !known.contains(&key) => Some(*at),
                _ => None,
            };
            if at.is_some() {
                arrival.take();
                state.raised.set(Some(key.clone()));
            }
            at
        });
        if let Some(at) = arrived {
            from.set(Some(at));
        }
    });
    let face = content.call(ReorderItemHandle {
        lifted,
        raised,
        draggable,
    });
    let api = ComponentRef::<GlideApi>::new();
    // While on a place here the row is held where the carry draws it; let go, it springs
    // to its slot.
    let holding = Cell::new(false);
    create_effect(move || {
        let frame = held.get().then(|| drag_board().frame()).flatten();
        if frame.is_none() && !holding.get() {
            return;
        }
        holding.set(frame.is_some());
        let along = Some(direction.get_untracked());
        untrack(|| {
            let at = frame.map(|frame| state.with(|state| clamped(state, frame.item)));
            api.get().hold(at.map(|at| GlideHold { at, along }))
        });
    });
    // A fixed list keeps its own row's slot open, empty, while the row's preview floats.
    let hidden = create_memo(move || lifted.get() && fixed.get());
    let root = NodeRef::new();
    // Where the carry was last drawn while the row was away, to glide back in from.
    let last = Rc::new(Cell::new(None::<Pos2>));
    {
        let last = Rc::clone(&last);
        create_effect(move || {
            if away.get()
                && let Some(frame) = drag_board().frame()
            {
                last.set(Some(frame.item.min));
            }
        });
    }
    create_effect_on_change(
        move || away.get(),
        move |away, was| {
            if away == was {
                return;
            }
            let main = direction.get_untracked();
            let length = root
                .try_get()
                .and_then(|root| measure(root).get_untracked())
                .map_or(0.0, |measured| main.main(measured.rect.size()));
            fold(state, key.get_untracked(), *away, length);
        },
    );
    // Uncollapsed after coming back, the row glides in from where it was let go.
    create_effect_on_change(
        move || collapsed.get(),
        move |collapsed, was| {
            if *was
                && !*collapsed
                && let Some(at) = last.take()
            {
                from.set(Some(at));
            }
        },
    );
    let clicked = move |_: PointerPress| {
        if lifted.get_untracked() {
            return;
        }
        on_click.call(());
    };
    let drag = Func::new(move |_: PointerPress| lift(state, key.get_untracked()));
    let (spring, lifts_on_long_press) = state.with(|state| (state.spring, state.lifts_on_long_press));
    let inner = view! {
        <Glide spring on={arm} within={Some(api)}>
            <Interactive
                cursor
                capture_presses
                touch_drags={move || match draggable.get() {
                    true => TouchDrags::Along(match direction.get() {
                        Direction::Horizontal => Direction::Vertical,
                        Direction::Vertical => Direction::Horizontal,
                    }),
                    false => TouchDrags::None,
                }}
                drag={Some(drag)}
                lifts_on_long_press
                on_click_at={clicked}
            >
                {face}
            </Interactive>
        </Glide>
    };
    let row = NodeRef::new();
    let glide = view! {
        <Frame @node_ref=&row collapsed={collapsed} raised={raised}>
            <Glide api spring on={arm} from={from}>
                <Frame padding={padding}>
                    <Frame
                        opacity={move || match hidden.get() {
                            true => 0.0,
                            false => 1.0,
                        }}
                    >
                        {inner}
                    </Frame>
                </Frame>
            </Glide>
        </Frame>
    };
    root.fill(glide);
    for node in [glide, inner, face] {
        measure(node);
    }
    state.with(|state| state.glides.borrow_mut().insert(glide, (inner, api.get())));
    // A row the list reports with other nodes than this glide is wired wrong: the `Reorder`
    // could not find it to measure or glide it.
    #[cfg(debug_assertions)]
    {
        let rows = reorder.rows;
        create_effect(move || {
            rows.keys();
            let key = key.get_untracked();
            let nodes = rows.nodes_of(&key);
            debug_assert!(
                nodes.is_empty() || nodes.contains(&glide),
                "a ReorderItem's glide is not among the nodes its list reports for its key; hand \
                 the list the ReorderItem as its row"
            );
        });
    }
    on_cleanup(move || {
        state.try_with(|state| state.glides.borrow_mut().remove(&glide));
    });
    on_key(
        KeyAt::Node(row),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let Some(press) = event.press() else {
                return false;
            };
            if !press.pressed
                || !press.modifiers.alt
                || press.modifiers.ctrl
                || press.modifiers.shift
            {
                return false;
            }
            let by = match (direction.get_untracked(), press.key) {
                (Direction::Vertical, Key::ArrowUp) | (Direction::Horizontal, Key::ArrowLeft) => -1,
                (Direction::Vertical, Key::ArrowDown)
                | (Direction::Horizontal, Key::ArrowRight) => 1,
                _ => return false,
            };
            step(state, key.get_untracked(), by)
        },
    );
    glide
}
