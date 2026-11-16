//! A place that takes dropped items, typed: what it accepts, the places a carried item can
//! land in it, and what committing a drop does. Its `over` is the place chosen in it while an
//! item is carried; it draws that, and writes nothing until `commit`.

#[cfg(test)]
mod tests;

use std::any::Any;
use std::rc::Rc;

use beui_view::drag::{After, Candidate, DragSpec, Space};
use beui_view::reactive::*;
use beui_view::types::{Direction, Pos2, Rect, Vec2};

use crate::drag_board::{Board, Erased, Site, drag_board};

/// A site's handle: the place chosen in it while an item is carried, else `None`.
pub struct DropSiteHandle<D: 'static> {
    pub over: ReadSignal<Option<D>>,
}

impl<D> Clone for DropSiteHandle<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for DropSiteHandle<D> {}

/// Makes the current component a drop site on the tree's board, for as long as its scope
/// lives: `accepts` says which payloads it takes, `spec` gives the places a carried one can
/// land in it (read once, at the lift, from the layout then), and `commit` does the drop.
pub fn use_drop_site<P, D>(
    accepts: Func<P, bool>,
    spec: Func<P, DragSpec<D>>,
    commit: Callback<(P, D)>,
) -> DropSiteHandle<D>
where
    P: Clone + 'static,
    D: Clone + 'static,
{
    drop_site_on(&drag_board(), accepts, spec, commit)
}

pub(crate) fn drop_site_on<P, D>(
    board: &Rc<Board>,
    accepts: Func<P, bool>,
    spec: Func<P, DragSpec<D>>,
    commit: Callback<(P, D)>,
) -> DropSiteHandle<D>
where
    P: Clone + 'static,
    D: Clone + 'static,
{
    let over = create_signal(None::<D>);
    let typed = |payload: &dyn Any| payload.downcast_ref::<P>().cloned();
    let id = board.add_site(Site {
        id: 0,
        accepts: Rc::new(move |payload| {
            typed(payload).is_some_and(|payload| accepts.call(payload))
        }),
        spec: Rc::new(move |payload| match typed(payload) {
            Some(payload) => spec
                .call(payload)
                .map(Rc::new(|drop: D| Rc::new(drop) as Erased)),
            None => DragSpec::Floating,
        }),
        over: Rc::new(move |drop: Option<Erased>| {
            // The board may tell a site whose scope is already gone, in whatever order a
            // teardown runs; it hears nothing then.
            if !over.is_alive() {
                return;
            }
            let drop = drop.and_then(|drop| drop.downcast_ref::<D>().cloned());
            over.update(|over| *over = drop);
        }),
        commit: Rc::new(move |payload: Erased, drop: Erased| {
            if let (Some(payload), Some(drop)) =
                (payload.downcast_ref::<P>(), drop.downcast_ref::<D>())
            {
                commit.call((payload.clone(), drop.clone()));
            }
        }),
    });
    let board = Rc::clone(board);
    on_cleanup(move || board.remove_site(id));
    DropSiteHandle {
        over: over.read_only(),
    }
}

/// The payload carried on the tree's board, if it is a `P`; tracked.
pub fn carrying<P: Clone + 'static>() -> Option<P> {
    drag_board()
        .carried()
        .and_then(|payload| payload.downcast_ref::<P>().cloned())
}

/// Whether `key` is what is carried; a row draws itself as the gap while it is.
pub fn is_carried<K: Clone + PartialEq + 'static>(key: K) -> Memo<bool> {
    create_memo(move || carrying::<K>().is_some_and(|carried| carried == key))
}

/// A place that is the whole of `node`, chosen while the pointer is on it (a slot, a folder,
/// a button that takes what is dropped on it): `None` while `node` is not laid out. A zone
/// holds no stickiness: of several that hold the pointer, one inside another, the first
/// listed in a `closest` wins whatever was chosen before, so list a nested one before the
/// one around it.
pub fn place_on<D>(node: NodeRef, drop: D) -> Option<Candidate<D>> {
    let (rect, scale) = node.try_get().and_then(|id| beui_view::drag::space().placed(id))?;
    let whole = Rect::from_min_size(Pos2::ZERO, rect.size() / scale);
    Some(Candidate::new(drop, whole, node).zoned(whole))
}

/// The places a carried row can land among `rows`, along `direction`, each named by the
/// row it would come after (`carried` is its key, or `None` for an item from elsewhere): read now from where the rows are laid out, relative to `list`
/// (the node they sit in), so a scroll or a moving window carries them along. A row lands at
/// the start of a row it passes going up, and at the end of one it passes going down, as the
/// rows between close up behind it; rows of any length rest exactly, and a row of several
/// children rests where all of them do.
///
/// An item from elsewhere, or a key not among the rows (a move between sites), gets a place at each gap between the rows
/// and one past the last, the size of the item carried, as the rows would part for it: the
/// list's `commit` takes it in (decisions/beui_a_move_between_drop_sites_is_the_apps.md). A
/// key shown more than once is carried as its first row; its other rows are left out.
pub fn slots_in<K>(
    list: NodeRef,
    rows: ForEachRef<K, NodeId>,
    carried: Option<&K>,
    direction: Direction,
) -> Vec<Candidate<After<K>>>
where
    K: Clone + PartialEq + 'static,
{
    let item = drag_board().carried_item().map(|item| item.size());
    slots_with(
        &*beui_view::drag::space(),
        list,
        rows,
        carried,
        direction,
        item,
    )
}

pub(crate) fn slots_with<K>(
    space: &dyn Space,
    list: NodeRef,
    rows: ForEachRef<K, NodeId>,
    carried: Option<&K>,
    direction: Direction,
    item: Option<Vec2>,
) -> Vec<Candidate<After<K>>>
where
    K: Clone + PartialEq + 'static,
{
    let Some((origin, scale)) = list.try_get().and_then(|list| space.placed(list)) else {
        return Vec::new();
    };
    let keys = untrack(|| rows.keys());
    // A row of several children rests where all of them do.
    let rect_of = |key: &K| {
        rows.nodes_of(key)
            .into_iter()
            .filter_map(|node| space.placed(node).map(|(rect, _)| rect))
            .reduce(|all, rect| all.union(rect))
    };
    let in_list = |window: Rect| {
        Rect::from_min_size(
            Pos2::ZERO + (window.min - origin.min) / scale,
            window.size() / scale,
        )
    };
    let Some((home, carried)) =
        carried.and_then(|carried| Some((keys.iter().position(|key| key == carried)?, carried)))
    else {
        let Some(item) = item else {
            return Vec::new();
        };
        return visiting(&keys, &rect_of, origin, direction, item)
            .into_iter()
            .map(|(after, window)| Candidate::new(After(after), in_list(window), list))
            .collect();
    };
    let Some(own) = rect_of(carried) else {
        return Vec::new();
    };
    let along = |pos: Pos2| direction.main(pos.to_vec2());
    let length = direction.main(own.size());
    let rest: Vec<&K> = keys.iter().filter(|key| *key != carried).collect();
    let mut slots = Vec::with_capacity(keys.len());
    for (at, key) in keys.iter().enumerate() {
        let Some(rect) = rect_of(key) else {
            continue;
        };
        // Where the carried row starts when it lands at `at`: the start of the row there going
        // up, the end of it less its own length going down.
        let start = match at <= home {
            true => along(rect.min),
            false => along(rect.max) - length,
        };
        let min = match direction {
            Direction::Vertical => Pos2::new(own.min.x, start),
            Direction::Horizontal => Pos2::new(start, own.min.y),
        };
        let window = Rect::from_min_size(min, own.size());
        let after = at.checked_sub(1).map(|before| rest[before].clone());
        slots.push(Candidate::new(After(after), in_list(window), list));
    }
    slots
}

/// Where an item of size `item` from another list rests at each gap among `keys`, in window
/// space: at the start of the row it lands before (the rows from there move on by its
/// length), or past the last row by the spacing between rows; at the list's start when it
/// has none.
fn visiting<K: Clone>(
    keys: &[K],
    rect_of: impl Fn(&K) -> Option<Rect>,
    list: Rect,
    direction: Direction,
    item: Vec2,
) -> Vec<(Option<K>, Rect)> {
    let along = |pos: Pos2| direction.main(pos.to_vec2());
    let at = |start: f32, cross: Pos2| match direction {
        Direction::Vertical => Pos2::new(cross.x, start),
        Direction::Horizontal => Pos2::new(start, cross.y),
    };
    let shown: Vec<(&K, Rect)> = keys
        .iter()
        .filter_map(|key| Some((key, rect_of(key)?)))
        .collect();
    let Some((_, first)) = shown.first() else {
        return vec![(None, Rect::from_min_size(list.min, item))];
    };
    let spacing = shown
        .windows(2)
        .map(|pair| along(pair[1].1.min) - along(pair[0].1.max))
        .filter(|between| *between >= 0.0)
        .reduce(f32::min)
        .unwrap_or(0.0);
    let mut places = Vec::with_capacity(shown.len() + 1);
    let mut after = None;
    for (key, rect) in &shown {
        places.push((
            after,
            Rect::from_min_size(at(along(rect.min), first.min), item),
        ));
        after = Some((*key).clone());
    }
    let end = shown
        .last()
        .map_or(0.0, |(_, rect)| along(rect.max) + spacing);
    places.push((after, Rect::from_min_size(at(end, first.min), item)));
    places
}
