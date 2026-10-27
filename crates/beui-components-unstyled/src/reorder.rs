use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Live, Node, Role};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::drag::{Board, DragPoint, DragSource, Pending, drag_board};
use beui_view::primitives::*;
use beui_view::reactive::*;
use beui_view::reactive::{KeyAt, NodeId, OverlayAnchor};
use beui_view::types::layout::GLIDE_SPRING;
use beui_view::types::{
    CursorIcon, Direction, GlideHold, Key, KeyPhase, LayerLevel, LayerPolicy, Placement,
    PointerPress, Pos2, Rect, Sides, TOUCH_DRAG_THRESHOLD, TouchDrags,
};

use crate::drag::{DRAG_THRESHOLD, DropHandle, DropTarget};

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
    pub lifted: Memo<bool>,
    pub raised: Memo<bool>,
    pub draggable: Memo<bool>,
}

#[derive(Clone, PartialEq)]
struct Lifted<K, P> {
    key: K,
    payload: P,
    item: Rect,
    home: bool,
    slot: usize,
    size: f32,
}

#[derive(Clone)]
struct Press<K> {
    key: K,
    origin: Pos2,
    touched: bool,
    item: Rect,
}

enum Arrival<K> {
    Back(K, Pos2),
    New(Vec<K>, Pos2),
}

struct State<K: 'static, P: 'static> {
    keys: Memo<Vec<K>>,
    direction: Prop<Direction>,
    payload: Func<K, Option<P>>,
    accepts: Option<Func<P, bool>>,
    disabled: Prop<bool>,
    fixed: Prop<bool>,
    long_press: Prop<bool>,
    text: Callback<K, String>,
    on_move: Callback<ReorderMove<K, P>>,
    on_drag_change: Callback<Option<K>>,
    on_drop_outside: Callback<(K, DragPoint)>,
    lifted: Signal<Option<Lifted<K, P>>>,
    gap: Signal<Option<ReorderGap<K>>>,
    aim: Cell<f32>,
    speed: Cell<f32>,
    frames: Cell<Option<Animation>>,
    visited: Signal<bool>,
    arm: Signal<u64>,
    raised: Signal<Option<K>>,
    floating_at: Signal<Option<DragPoint>>,
    announcement: Signal<String>,
    point: RefCell<Option<DragPoint>>,
    pressed: RefCell<Option<Press<K>>>,
    arrival: RefCell<Option<Arrival<K>>>,
    glides: RefCell<Vec<(K, NodeId, NodeId)>>,
    board: Board,
    gone: Cell<bool>,
    returning: Cell<bool>,
    area: Cell<Option<NodeId>>,
}

pub struct ReorderHandle<K: 'static, P: 'static> {
    state: Stored<State<K, P>>,
    pub order: Memo<Vec<K>>,
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

    pub fn drop_at(&self, payload: P, point: DragPoint) {
        dropped_in(self.state, payload, point);
    }

    pub fn draggable(&self, key: K) -> bool {
        self.state.with(|state| can_drag(state, &key))
    }

    pub fn resting(&self, key: &K) -> Option<Rect> {
        self.state.with(|state| {
            let glides = state.glides.borrow();
            let (_, outer, inner) = glides.iter().find(|(shown, ..)| shown == key)?;
            resting(*outer, *inner)
        })
    }
}

fn along(direction: Direction, pos: Pos2) -> f32 {
    direction.main(pos.to_vec2())
}

fn can_drag<K: Clone, P>(state: &State<K, P>, key: &K) -> bool {
    !state.disabled.get_untracked() && state.payload.call(key.clone()).is_some()
}

fn bump<K, P>(state: &State<K, P>) {
    state.arm.update(|arm| *arm += 1);
}

fn glide_of<K: PartialEq, P>(state: &State<K, P>, key: &K) -> Option<NodeId> {
    state
        .glides
        .borrow()
        .iter()
        .find(|(shown, ..)| shown == key)
        .map(|(_, node, _)| *node)
}

fn resting(outer: NodeId, inner: NodeId) -> Option<Rect> {
    let rect = measure(inner).get_untracked()?.rect;
    let moved = glide_displacement(outer)?;
    Some(rect.translate(-moved))
}

fn hold<K: Clone + PartialEq, P: Clone>(state: &State<K, P>, held: bool) {
    let Some(lifted) = state.lifted.get_untracked() else {
        return;
    };
    let Some(glide) = glide_of(state, &lifted.key) else {
        return;
    };
    let point = *state.point.borrow();
    let direction = state.direction.get_untracked();
    let hold = point
        .filter(|_| held && lifted.home)
        .map(|point| GlideHold {
            at: clamped(state, point.pos + lifted.item.min.to_vec2(), lifted.item),
            along: Some(direction),
        });
    hold_glide(glide, hold);
}

fn spans<K, P>(state: &State<K, P>, except: Option<&K>) -> Vec<(f32, f32)>
where
    K: PartialEq,
{
    let direction = state.direction.get_untracked();
    let mut spans: Vec<(f32, f32)> = state
        .glides
        .borrow()
        .iter()
        .filter(|(key, ..)| except != Some(key))
        .filter_map(|(_, outer, _)| {
            measure(*outer)
                .get_untracked()
                .map(|measured| measured.rect)
        })
        .map(|rect| (along(direction, rect.min), along(direction, rect.max)))
        .collect();
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    spans
}

fn clamped<K, P>(state: &State<K, P>, at: Pos2, item: Rect) -> Pos2 {
    let Some(area) = state
        .area
        .get()
        .and_then(|area| measure(area).get_untracked())
        .map(|measured| measured.rect)
    else {
        return at;
    };
    let direction = state.direction.get_untracked();
    let low = along(direction, area.min);
    let high = (along(direction, area.max) - direction.main(item.size())).max(low);
    match direction {
        Direction::Horizontal => Pos2::new(at.x.clamp(low, high), at.y),
        Direction::Vertical => Pos2::new(at.x, at.y.clamp(low, high)),
    }
}

fn spacing<K: PartialEq, P>(state: &State<K, P>) -> f32 {
    spans(state, None)
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].1)
        .filter(|between| *between >= 0.0)
        .reduce(f32::min)
        .unwrap_or(0.0)
}

fn slot_size<K: PartialEq, P>(state: &State<K, P>, key: &K) -> f32 {
    let direction = state.direction.get_untracked();
    let Some((start, end)) = state
        .glides
        .borrow()
        .iter()
        .find(|(shown, ..)| shown == key)
        .and_then(|(_, outer, _)| {
            measure(*outer)
                .get_untracked()
                .map(|measured| measured.rect)
        })
        .map(|rect| (along(direction, rect.min), along(direction, rect.max)))
    else {
        return 0.0;
    };
    let others = spans(state, Some(key));
    match others.iter().find(|span| span.0 >= end) {
        Some(next) => next.0 - start,
        None => {
            let previous = others
                .iter()
                .filter(|span| span.1 <= start)
                .map(|span| span.1)
                .reduce(f32::max);
            end - previous.unwrap_or(start)
        }
    }
}

fn carried_size<K: PartialEq, P>(state: &State<K, P>, point: DragPoint) -> f32 {
    state.direction.get_untracked().main(point.item.size()) + spacing(state)
}

fn shown_keys<K: Clone + PartialEq, P: Clone>(state: &State<K, P>) -> Vec<K> {
    let keys = state.keys.get_untracked();
    match state.lifted.get_untracked() {
        Some(lifted) => rest_of(&keys, &lifted.key),
        None => keys,
    }
}

fn shown_at<K: Clone + PartialEq, P: Clone>(state: &State<K, P>) -> Option<Pos2> {
    let lifted = state.lifted.get_untracked()?;
    let point = (*state.point.borrow())?;
    let at = point.pos + lifted.item.min.to_vec2();
    let resting = glide_of(state, &lifted.key)
        .and_then(|glide| measure(glide).get_untracked().map(|measured| measured.rect));
    Some(match (lifted.home, resting) {
        (true, Some(resting)) => match state.direction.get_untracked() {
            Direction::Horizontal => Pos2::new(at.x, resting.min.y),
            Direction::Vertical => Pos2::new(resting.min.x, at.y),
        },
        _ => at,
    })
}

fn slot_at<K: Clone + PartialEq, P>(
    state: &State<K, P>,
    rest: &[K],
    own: Option<&K>,
    span: (f32, f32),
    fallback: usize,
    full: f32,
) -> usize {
    let direction = state.direction.get_untracked();
    let glides = state.glides.borrow();
    let mut mounted: Vec<(f32, usize)> = Vec::new();
    let mut split = f32::INFINITY;
    let mut opened = 0.0;
    for (key, outer, inner) in glides.iter() {
        let Some(rect) = resting(*outer, *inner) else {
            continue;
        };
        if own == Some(key) {
            split = split.min(along(direction, rect.center()));
            continue;
        }
        let Some(index) = rest.iter().position(|shown| shown == key) else {
            continue;
        };
        let start = along(direction, rect.min);
        let lead = measure(*outer)
            .get_untracked()
            .map(|measured| measured.rect)
            .map_or(start, |row| along(direction, row.min));
        if lead < start {
            split = split.min(lead);
            opened = start - lead;
        }
        mounted.push((along(direction, rect.center()), index));
    }
    drop(glides);
    mounted.sort_by(|left, right| left.0.total_cmp(&right.0));
    let (low, high) = span;
    let passed = |(middle, _): &(f32, usize)| match *middle < split {
        true => low >= *middle,
        false => high >= *middle - opened + full,
    };
    if let Some((_, index)) = mounted.iter().find(|entry| !passed(entry)) {
        return *index;
    }
    match mounted.last() {
        Some((_, index)) => index + 1,
        None if rest.is_empty() => 0,
        None => fallback.min(rest.len()),
    }
}

fn span_of(direction: Direction, point: DragPoint) -> (f32, f32) {
    let rect = point.item.translate(point.pos.to_vec2());
    (along(direction, rect.min), along(direction, rect.max))
}

fn rest_of<K: Clone + PartialEq>(keys: &[K], key: &K) -> Vec<K> {
    keys.iter().filter(|shown| *shown != key).cloned().collect()
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

fn place<K, P>(stored: Stored<State<K, P>>, point: DragPoint)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    stored.with(|state| {
        *state.point.borrow_mut() = Some(point);
        let Some(mut lifted) = state.lifted.get_untracked().filter(|lifted| lifted.home) else {
            return;
        };
        if !state.fixed.get_untracked() {
            let rest = rest_of(&state.keys.get_untracked(), &lifted.key);
            let direction = state.direction.get_untracked();
            let span = span_of(direction, point);
            let slot = slot_at(state, &rest, Some(&lifted.key), span, lifted.slot, 0.0);
            if lifted.slot != slot {
                lifted.slot = slot;
                bump(state);
                state.lifted.set(Some(lifted));
            }
        }
        hold(state, true);
    });
}

fn visit<K, P>(stored: Stored<State<K, P>>, point: Option<DragPoint>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let (own, gap) = stored.with(|state| {
        let own = state.lifted.with_untracked(Option::is_some);
        let gap = point.filter(|_| !state.fixed.get_untracked()).map(|point| {
            let keys = shown_keys(state);
            let direction = state.direction.get_untracked();
            let size = carried_size(state, point);
            let span = span_of(direction, point);
            let index = slot_at(state, &keys, None, span, keys.len(), size);
            ReorderGap {
                before: keys.get(index).cloned(),
                size,
            }
        });
        (own, gap)
    });
    open_gap(stored, point.is_some() && !own, gap);
}

fn open_gap<K, P>(stored: Stored<State<K, P>>, visited: bool, gap: Option<ReorderGap<K>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let frames = stored.with(|state| {
        if state.visited.get_untracked() != visited {
            state.visited.set(visited);
        }
        let gap = gap.filter(|_| !state.fixed.get_untracked());
        let aim = gap.as_ref().map_or(0.0, |gap| gap.size);
        match (gap, state.gap.get_untracked()) {
            (Some(gap), Some(shown)) if gap.before != shown.before => {
                bump(state);
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
            GLIDE_SPRING.advance(&mut offset, &mut speed, seconds);
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
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let back = stored.try_with(|state| {
        state.speed.set(0.0);
        let aim = state.aim.get();
        if let Some(mut shown) = state.gap.get_untracked()
            && shown.size != aim
        {
            shown.size = aim;
            state.gap.set((aim > 0.0).then_some(shown));
        }
        state.returning.replace(false)
    });
    if back == Some(true) {
        come_back(stored);
    }
}

fn come_back<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    stored.with(|state| shut(state));
    finish(stored);
}

fn cancel<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let home = stored.with(|state| {
        state.pressed.borrow_mut().take();
        state.board.release();
        let lifted = state
            .lifted
            .get_untracked()
            .filter(|lifted| !lifted.home && lifted.size > 0.0 && !state.fixed.get_untracked())?;
        let keys = state.keys.get_untracked();
        let at = keys.iter().position(|key| *key == lifted.key)?;
        let rest = rest_of(&keys, &lifted.key);
        state.returning.set(true);
        Some(ReorderGap {
            before: rest.get(at).cloned(),
            size: lifted.size,
        })
    });
    let Some(home) = home else {
        come_back(stored);
        return;
    };
    open_gap(stored, false, Some(home));
    let waiting = stored.with(|state| {
        state.returning.get() && state.frames.get().is_some_and(|frames| frames.running())
    });
    if !waiting && stored.with(|state| state.returning.replace(false)) {
        come_back(stored);
    }
}

fn detach<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let frames = stored.with(|state| {
        let mut lifted = state.lifted.get_untracked().filter(|lifted| lifted.home)?;
        let rest = rest_of(&state.keys.get_untracked(), &lifted.key);
        let size = slot_size(state, &lifted.key);
        hold(state, false);
        lifted.home = false;
        lifted.size = size;
        state.floating_at.set(*state.point.borrow());
        let before = rest.get(lifted.slot).cloned();
        state.lifted.set(Some(lifted));
        state.aim.set(0.0);
        state.speed.set(0.0);
        state
            .gap
            .set((size > 0.0).then_some(ReorderGap { before, size }));
        state.frames.get()
    });
    if let Some(frames) = frames {
        frames.start();
    }
}

fn finish<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    stored.with(|state| {
        let Some(lifted) = state.lifted.get_untracked() else {
            return;
        };
        hold(state, false);
        if !lifted.home
            && !state.fixed.get_untracked()
            && let Some(at) = shown_at(state)
        {
            *state.arrival.borrow_mut() = Some(Arrival::Back(lifted.key.clone(), at));
        }
        bump(state);
        if state.pressed.borrow_mut().take().is_some() {
            state.board.release();
        }
        state.floating_at.set(None);
        state.lifted.set(None);
        state.on_drag_change.call(None);
    });
}

fn dropped_home<K, P>(stored: Stored<State<K, P>>, point: DragPoint)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    place(stored, point);
    let commit = stored.with(|state| {
        let lifted = state.lifted.get_untracked()?;
        let keys = state.keys.get_untracked();
        let rest = rest_of(&keys, &lifted.key);
        let slot = lifted.slot.min(rest.len());
        let from = keys.iter().position(|key| *key == lifted.key);
        announce(state, &lifted.key, slot, keys.len());
        (from != Some(slot)).then(|| ReorderMove {
            payload: lifted.payload.clone(),
            key: Some(lifted.key.clone()),
            after: slot.checked_sub(1).and_then(|at| rest.get(at).cloned()),
            index: slot,
        })
    });
    if let Some(commit) = commit {
        stored.with(|state| state.on_move.call(commit));
    }
    finish(stored);
}

fn dropped_in<K, P>(stored: Stored<State<K, P>>, payload: P, point: DragPoint)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let commit = stored.with(|state| {
        let keys = state.keys.get_untracked();
        let own = state.lifted.get_untracked().map(|lifted| lifted.key);
        let rest = match &own {
            Some(own) => rest_of(&keys, own),
            None => keys.clone(),
        };
        let from = own
            .as_ref()
            .and_then(|own| keys.iter().position(|key| key == own));
        let direction = state.direction.get_untracked();
        let index = match state.fixed.get_untracked() {
            true => from.unwrap_or(0),
            false => {
                let size = carried_size(state, point);
                let span = span_of(direction, point);
                slot_at(state, &rest, None, span, rest.len(), size)
            }
        };
        let at = point.pos + point.item.min.to_vec2();
        *state.arrival.borrow_mut() = Some(match &own {
            Some(own) => Arrival::Back(own.clone(), at),
            None => Arrival::New(keys.clone(), at),
        });
        shut(state);
        state.visited.set(false);
        bump(state);
        if let Some(own) = &own {
            announce(state, own, index, keys.len());
        }
        (from != Some(index)).then(|| ReorderMove {
            payload,
            key: own,
            after: index.checked_sub(1).and_then(|at| rest.get(at).cloned()),
            index,
        })
    });
    if let Some(commit) = commit {
        stored.with(|state| state.on_move.call(commit));
    }
}

fn step<K, P>(stored: Stored<State<K, P>>, key: K, by: isize) -> bool
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let commit = stored.with(|state| {
        if !can_drag(state, &key)
            || state.fixed.get_untracked()
            || state.lifted.with_untracked(Option::is_some)
        {
            return None;
        }
        let keys = state.keys.get_untracked();
        let from = keys.iter().position(|shown| *shown == key)?;
        let to = from.saturating_add_signed(by).min(keys.len() - 1);
        if to == from {
            return None;
        }
        let rest = rest_of(&keys, &key);
        bump(state);
        state.raised.set(Some(key.clone()));
        announce(state, &key, to, keys.len());
        Some(ReorderMove {
            payload: state.payload.call(key.clone())?,
            key: Some(key.clone()),
            after: to.checked_sub(1).and_then(|at| rest.get(at).cloned()),
            index: to,
        })
    });
    let Some(commit) = commit else {
        return false;
    };
    stored.with(|state| state.on_move.call(commit));
    true
}

fn lift<K, P>(stored: Stored<State<K, P>>, press: Press<K>, point: DragPoint)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    if stored.with(|state| state.returning.replace(false)) {
        come_back(stored);
    }
    let Some((payload, board)) = stored.with(|state| {
        if !can_drag(state, &press.key) || state.board.carrying() {
            return None;
        }
        let payload = state.payload.call(press.key.clone())?;
        let keys = state.keys.get_untracked();
        let slot = keys.iter().position(|key| *key == press.key)?;
        state.arrival.borrow_mut().take();
        *state.point.borrow_mut() = Some(point);
        state.raised.set(Some(press.key.clone()));
        let fixed = state.fixed.get_untracked();
        state.lifted.set(Some(Lifted {
            key: press.key.clone(),
            payload: payload.clone(),
            item: press.item,
            home: !fixed,
            slot,
            size: 0.0,
        }));
        if fixed {
            state.floating_at.set(Some(point));
        }
        state.on_drag_change.call(Some(press.key.clone()));
        Some((payload, state.board.clone()))
    }) else {
        return;
    };
    board.begin(
        DragSource {
            item: press.item,
            follow: Rc::new(move |point: DragPoint| {
                stored.with(|state| {
                    *state.point.borrow_mut() = Some(point);
                    let home = state
                        .lifted
                        .with_untracked(|lifted| lifted.as_ref().is_some_and(|lifted| lifted.home));
                    if !home {
                        state.floating_at.set(Some(point));
                    }
                });
            }),
            floating: Rc::new(move |floating: bool| {
                if floating {
                    detach(stored);
                }
            }),
            outside: Rc::new(move |point: DragPoint| {
                let key =
                    stored.with(|state| state.lifted.get_untracked().map(|lifted| lifted.key));
                finish(stored);
                if let Some(key) = key {
                    stored.with(|state| state.on_drop_outside.call((key, point)));
                }
            }),
            cancelled: Rc::new(move || {
                if stored.try_with(|state| state.gone.get()) != Some(false) {
                    return;
                }
                cancel(stored);
            }),
            dropped: Rc::new(move || {
                if stored.try_with(|state| state.gone.get()) == Some(false) {
                    finish(stored);
                }
            }),
            ..DragSource::new(Rc::new(payload))
        },
        point,
    );
}

fn moved<K, P>(stored: Stored<State<K, P>>, point: DragPoint)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let (lifted, press) = stored.with(|state| {
        (
            state.lifted.with_untracked(Option::is_some),
            state.pressed.borrow().clone(),
        )
    });
    if lifted {
        stored.with(|state| state.board.track(point));
        return;
    }
    let Some(press) = press else {
        return;
    };
    let threshold = match press.touched {
        true => TOUCH_DRAG_THRESHOLD,
        false => DRAG_THRESHOLD,
    };
    if (point.pos - press.origin).length() < threshold {
        return;
    }
    lift(stored, press, point);
}

fn released<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let board = stored.with(|state| {
        state.pressed.borrow_mut().take();
        state.board.release();
        state
            .lifted
            .with_untracked(Option::is_some)
            .then(|| state.board.clone())
    });
    if let Some(board) = board {
        board.finish();
    }
    if stored.with(|state| state.lifted.with_untracked(Option::is_some)) {
        finish(stored);
    }
}

fn pointer_cancelled<K, P>(stored: Stored<State<K, P>>)
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let board = stored.with(|state| {
        state.pressed.borrow_mut().take();
        state.board.release();
        state
            .lifted
            .with_untracked(Option::is_some)
            .then(|| state.board.clone())
    });
    if let Some(board) = board {
        board.cancel();
    }
}

#[component]
pub fn Reorder<K, P>(
    keys: Prop<Vec<K>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    payload: Func<K, Option<P>>,
    #[prop(default = None)] accepts: Option<Func<P, bool>>,
    #[prop(default = None)] takes: Option<Func<DragPoint, bool>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] fixed: Prop<bool>,
    #[prop(default = false)] long_press: Prop<bool>,
    text: Callback<K, String>,
    on_move: Callback<ReorderMove<K, P>>,
    on_drag_change: Callback<Option<K>>,
    on_drop_outside: Callback<(K, DragPoint)>,
    preview: RenderFn<P>,
    #[prop(children)] content: Render<ReorderHandle<K, P>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let keys = create_memo(move || keys.get());
    let lifted = create_signal(None::<Lifted<K, P>>);
    let gap = create_signal(None::<ReorderGap<K>>);
    let visited = create_signal(false);
    let floating_at = create_signal(None::<DragPoint>);
    let announcement = create_signal(String::new());
    let state = store(State {
        keys,
        direction,
        payload,
        accepts,
        disabled,
        fixed,
        long_press,
        text,
        on_move,
        on_drag_change,
        on_drop_outside,
        lifted,
        gap,
        aim: Cell::new(0.0),
        speed: Cell::new(0.0),
        frames: Cell::new(None),
        visited,
        arm: create_signal(0),
        raised: create_signal(None),
        floating_at,
        announcement,
        point: RefCell::new(None),
        pressed: RefCell::new(None),
        arrival: RefCell::new(None),
        glides: RefCell::new(Vec::new()),
        board: drag_board(),
        gone: Cell::new(false),
        returning: Cell::new(false),
        area: Cell::new(None),
    });
    let placed = create_memo(move || {
        lifted.with(|lifted| {
            lifted
                .as_ref()
                .map(|lifted| (lifted.key.clone(), lifted.home, lifted.slot))
        })
    });
    let order = create_memo(move || {
        let keys = keys.get();
        match placed.get() {
            Some((key, home, slot)) if keys.contains(&key) && !fixed.get() => {
                let mut rest = rest_of(&keys, &key);
                if home {
                    rest.insert(slot.min(rest.len()), key);
                }
                rest
            }
            _ => keys,
        }
    });
    let dragging = create_memo(move || placed.get().map(|(key, ..)| key));
    let frames = create_animation(
        move |frame| step_gap(state, frame.delta.as_secs_f32()),
        move || settle_gap(state),
    );
    state.with(|state| state.frames.set(Some(frames)));
    let handle = ReorderHandle {
        state,
        order,
        dragging,
        gap: create_memo(move || gap.get()),
        visited: create_memo(move || visited.get()),
        direction,
    };
    on_cleanup(move || {
        let carried = state.try_with(|state| {
            state.gone.set(true);
            if state.pressed.borrow_mut().take().is_some() {
                state.board.release();
            }
            state
                .lifted
                .with_untracked(Option::is_some)
                .then(|| state.board.clone())
        });
        if let Some(Some(board)) = carried {
            board.end(None);
        }
    });

    let face = content.call(handle);
    let floating = create_memo(move || floating_at.with(Option::is_some));
    let anchor = create_memo(move || {
        let item = lifted.with(|lifted| lifted.as_ref().map_or(Rect::ZERO, |lifted| lifted.item));
        floating_at
            .get()
            .map_or(Pos2::ZERO, |point| point.pos + item.min.to_vec2())
    })
    .into_prop()
    .map(OverlayAnchor::Point);
    let shown_in = create_memo(move || floating_at.get().map(|point| point.window));
    let carried = create_memo(move || {
        lifted.with(|lifted| lifted.as_ref().map(|lifted| lifted.payload.clone()))
    });
    let accepting = Func::new(move |payload: P| {
        state.with(|state| {
            state.lifted.with_untracked(Option::is_some)
                || (!state.disabled.get_untracked()
                    && state.accepts.is_some_and(|accepts| accepts.call(payload)))
        })
    });
    let taking = Func::new(move |point: DragPoint| takes.is_none_or(|takes| takes.call(point)));
    let home =
        move || lifted.with_untracked(|lifted| lifted.as_ref().is_some_and(|lifted| lifted.home));
    let hides = create_memo(move || {
        lifted.with(|lifted| lifted.as_ref().is_some_and(|lifted| lifted.home))
    });
    let holding =
        Func::new(move |_: P| home() && state.with(|state| state.on_drop_outside.is_empty()));
    let area = view! {
        <DropTarget
            accepts={accepting}
            takes={taking}
            hides_preview={hides}
            holds={holding}
            on_over={move |over: Option<(P, DragPoint)>| {
                match (over, home()) {
                    (Some((_, point)), true) => place(state, point),
                    (over, false) => visit(state, over.map(|(_, point)| point)),
                    (None, true) => {}
                }
            }}
            on_drop={move |(payload, point): (P, DragPoint)| {
                match home() {
                    true => dropped_home(state, point),
                    false => dropped_in(state, payload, point),
                }
            }}
        >|_: DropHandle|
            <Layers>
                {face}
                <ReorderStatus announcement={announcement.read_only()} />
                <Overlay
                    anchor={anchor}
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Inert
                    level=LayerLevel::Top
                    traps_focus=false
                    shown_in={shown_in}
                    open={floating}
                >
                    <List spacing=0.0>
                        <Dynamic value={carried}>|carried: Option<P>| {
                            match carried {
                                Some(carried) => preview.call(carried),
                                None => view! {
                                    <Frame />
                                },
                            }
                        }</Dynamic>
                    </List>
                </Overlay>
            </Layers>
        </DropTarget>
    };
    measure(area);
    state.with(|state| state.area.set(Some(area)));
    area
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
    on_click: ClickCallback,
    #[prop(children)] content: Render<ReorderItemHandle>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
    P: Clone + PartialEq + 'static,
{
    let state = reorder.state;
    let direction = reorder.direction;
    let key = create_memo(move || item.get());
    let (lifted_key, raised_key, gap, arm) =
        state.with(|state| (state.lifted, state.raised, state.gap, state.arm));
    let lifted = create_memo(move || {
        lifted_key.with(|lifted| {
            lifted
                .as_ref()
                .is_some_and(|lifted| lifted.home && key.with(|key| *key == lifted.key))
        })
    });
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
    let trail = create_memo(move || {
        gap.with(|gap| match gap {
            Some(ReorderGap { before: None, size })
                if order.with(|order| order.last() == Some(&key.get())) =>
            {
                *size
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
    let from = create_signal(None::<Pos2>);
    create_effect(move || {
        let key = key.get();
        let arrived = state.with(|state| {
            let mut arrival = state.arrival.borrow_mut();
            let at = match arrival.as_ref() {
                Some(Arrival::Back(back, at)) if *back == key => Some(*at),
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
    let pending: Pending = Rc::new(move |point: DragPoint| moved(state, point));
    let pressed = move |press: PointerPress| {
        let key = key.get_untracked();
        state.with(|state| {
            if !can_drag(state, &key) {
                return;
            }
            let shown = measure(face).get_untracked().map(|measured| measured.rect);
            *state.pressed.borrow_mut() = Some(Press {
                key,
                origin: press.pos,
                touched: press.touch,
                item: shown.map_or(Rect::ZERO, |rect| rect.translate(Pos2::ZERO - press.pos)),
            });
            if !press.touch {
                state.board.press(Rc::clone(&pending));
            }
        });
    };
    let long_pressed = move |press: PointerPress| {
        let held = state.with(|state| state.long_press.get_untracked());
        let Some(pressing) = state.with(|state| state.pressed.borrow().clone()) else {
            return;
        };
        if held {
            lift(state, pressing, DragPoint::of(press));
        }
    };
    let clicked = move |_: PointerPress| {
        if state.with(|state| state.lifted.with_untracked(Option::is_none)) {
            on_click.call();
        }
    };
    let across = create_memo(move || match draggable.get() {
        true => TouchDrags::Along(match direction.get() {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal,
        }),
        false => TouchDrags::None,
    });
    let long_press = state.with(|state| state.long_press);
    let on_long_press = match long_press {
        Prop::Static(false) => Callback::empty(),
        _ => Callback::new(long_pressed),
    };
    let inner = view! {
        <Glide arm={arm}>
            <Interactive
                cursor
                capture_presses
                touch_drags={across}
                on_press={pressed}
                on_drag={move |press: PointerPress| moved(state, DragPoint::of(press))}
                on_long_press_drag={on_long_press}
                on_click_at={clicked}
                on_cancel={move || pointer_cancelled(state)}
                on_active_change={move |active: bool| if !active { released(state) }}
            >
                {face}
            </Interactive>
        </Glide>
    };
    let row = NodeRef::new();
    let glide = view! {
        <Glide @node_ref=&row arm={arm} from={from}>
            <Frame padding={padding}>{inner}</Frame>
        </Glide>
    };
    for node in [glide, inner, face] {
        measure(node);
    }
    create_effect(move || {
        let key = key.get();
        state.with(|state| {
            let mut glides = state.glides.borrow_mut();
            glides.retain(|(_, node, _)| *node != glide);
            glides.push((key, glide, inner));
        });
        if untrack(|| lifted.get()) {
            state.with(|state| hold(state, true));
        }
    });
    on_cleanup(move || {
        state.try_with(|state| {
            state
                .glides
                .borrow_mut()
                .retain(|(_, node, _)| *node != glide)
        });
    });
    on_key(KeyAt::Node(row), KeyPhase::Bubble, move |event| {
        let Some(press) = event.press() else {
            return false;
        };
        if !press.pressed || !press.modifiers.alt || press.modifiers.ctrl || press.modifiers.shift {
            return false;
        }
        let by = match (direction.get_untracked(), press.key) {
            (Direction::Vertical, Key::ArrowUp) | (Direction::Horizontal, Key::ArrowLeft) => -1,
            (Direction::Vertical, Key::ArrowDown) | (Direction::Horizontal, Key::ArrowRight) => 1,
            _ => return false,
        };
        step(state, key.get_untracked(), by)
    });
    create_effect(move || {
        let raised = raised.get();
        set_raised(glide, raised);
    });
    glide
}
