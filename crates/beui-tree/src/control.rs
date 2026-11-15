use std::cell::{Cell, RefCell};
use std::hash::Hash;

use be_lib_stable_hash::HashMap;
use std::rc::Rc;

use beui_macros::component;
use beui_reactive::{
    Boundary, Get, KeyedItems, Memo, Pending, ReadSignal, Scope, Set, Signal, Stored, StoredCell,
    Trigger, create_binding, create_memo, create_signal, create_trigger, provide_context, store,
    untrack, use_context,
};

use crate::child_list::SlotId;
use crate::children::{
    Branch, ChildSlot, Children, Count, Counted, DynamicSegment, Fallback, FormOf, NestedHost, Or,
    SlotChild, SlotHost, ValueFallback,
};
use crate::component::{MissingProp, Render, RenderFn};
use crate::prop::Prop;

type Built<C> = RenderFn<(), Children<C>>;

fn built<C: SlotChild, H: 'static, B: Branch<C>>(
    render: RenderFn<H, B>,
) -> RenderFn<H, Children<C>> {
    RenderFn::new(move |handle| render.call(handle).into_children())
}

/// Builds `branch` in a scope of its own and puts what it builds in `slot`, replacing what
/// was there.
fn mount<C: SlotChild>(slot: &ChildSlot<C>, branch: impl FnOnce() -> Children<C>) -> Scope {
    let scope = Scope::detached();
    scope.context().run(|| branch().mount_in(slot.clone()));
    scope
}

type Kept<C> = Option<(Rc<NestedHost<C>>, Scope)>;

/// What a `Show` shows on: a `bool`, or an `Option`, whose value it hands its children as a
/// signal while it is `Some`, so a new value updates them in place.
#[diagnostic::on_unimplemented(
    message = "a `Show` shows on a `bool` or an `Option`, and its value is a `{Self}`",
    label = "give a `bool` or an `Option`: a `Match` picks among other values"
)]
pub trait ShowValue: Clone + 'static {
    /// What the children are handed: nothing, or a signal of what is in the `Option`.
    type Handed: 'static;
    type Inner: 'static;

    /// What is shown, or `None` while nothing is.
    fn split(self) -> Option<Self::Inner>;

    fn hand(inner: Self::Inner) -> (Self::Handed, Option<Signal<Self::Inner>>);

    fn refresh(signal: Signal<Self::Inner>, inner: Self::Inner);
}

impl ShowValue for bool {
    type Handed = ();
    type Inner = ();

    fn split(self) -> Option<()> {
        self.then_some(())
    }

    fn hand(_inner: ()) -> ((), Option<Signal<()>>) {
        ((), None)
    }

    fn refresh(_signal: Signal<()>, _inner: ()) {}
}

impl<T: Clone + 'static> ShowValue for Option<T> {
    type Handed = ReadSignal<T>;
    type Inner = T;

    fn split(self) -> Option<T> {
        self
    }

    fn hand(inner: T) -> (ReadSignal<T>, Option<Signal<T>>) {
        let signal = create_signal(inner);
        (signal.read_only(), Some(signal))
    }

    fn refresh(signal: Signal<T>, inner: T) {
        signal.set_unconditionally(inner);
    }
}

/// Shows its children while `value` is `true` or `Some`, and `fallback`, if given, while it
/// is not. Given an `Option`, the children are handed a signal of what is in it, which
/// changes in place while it stays `Some`: `<Show value={picked}>|item: ReadSignal<Item>|`.
/// By default a branch is built each time it is shown and dropped when it is not. With
/// `keep_alive` each is built once, the first time it shows, and kept while hidden, paused,
/// so nothing in it runs until it shows again.
///
/// A `Show` fills a run among its parent's children. When it has a `fallback` and both
/// branches build exactly one child, it also fills a slot that takes one `Child`; without a
/// `fallback`, one that takes an optional child (`Option<Child>`).
#[component]
pub fn Show<V, C, B, F>(
    value: Prop<V>,
    #[prop(default = false)] keep_alive: bool,
    fallback: F,
    #[prop(children)] then: RenderFn<V::Handed, B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    V: ShowValue,
    C: SlotChild,
    B: Branch<C>,
    F: Fallback<C>,
    <B as Counted>::Count: Or<F::Count>,
{
    let then = built(then);
    let fallback = fallback.into_render();
    let value = store(value);
    let handed = StoredCell::new(None::<Signal<V::Inner>>);
    let shown = RenderFn::new(move |()| {
        let inner = V::split(value.with(Prop::get_untracked))
            .expect("a `Show` builds its children while it has a value");
        let (handed_now, signal) = V::hand(inner);
        handed.set(signal);
        then.call(handed_now)
    });
    let visible = Prop::dynamic(move || V::split(value.with(Prop::get)).is_some());
    let refresh = move || {
        if let (Some(signal), Some(inner)) =
            (handed.get(), V::split(value.with(Prop::get_untracked)))
        {
            V::refresh(signal, inner);
        }
    };
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| match keep_alive {
            false => show(visible, [fallback, Some(shown)], refresh, slot),
            true => keep(visible, [fallback, Some(shown)], refresh, slot),
        },
    ))
}

/// `refresh` gives what is shown its value again when the value changes and it stays shown.
fn show<C: SlotChild>(
    condition: Prop<bool>,
    branches: [Option<Built<C>>; 2],
    refresh: impl Fn() + 'static,
    slot: ChildSlot<C>,
) {
    let held = store(RefCell::new(None::<(bool, Option<Scope>)>));
    create_binding(move || {
        let visible = condition.get();
        let held = held.value();
        let mut held = held.borrow_mut();
        if held.as_ref().is_some_and(|(shown, _)| *shown == visible) {
            if visible {
                untrack(&refresh);
            }
            return;
        }
        let built = branches[usize::from(visible)].map(|branch| mount(&slot, || branch.call(())));
        if built.is_none() {
            slot.fill(Vec::new());
        }
        drop(held.replace((visible, built)));
    });
}

/// Whether the branch of a `Show` with `keep_alive` around the caller is shown, and every such
/// branch around that: `None` outside any. What a kept branch holds outside itself while hidden (a
/// dock it joined) lets go by it, and takes back when it shows again.
pub fn kept_shown() -> Option<Memo<bool>> {
    use_context::<KeptShown>().map(|KeptShown(shown)| shown)
}

#[derive(Clone, Copy)]
struct KeptShown(Memo<bool>);

fn keep<C: SlotChild>(
    condition: Prop<bool>,
    branches: [Option<Built<C>>; 2],
    refresh: impl Fn() + 'static,
    slot: ChildSlot<C>,
) {
    let held = store(RefCell::new([None, None] as [Kept<C>; 2]));
    let shown = StoredCell::new(None::<bool>);
    let outer = kept_shown();
    let showing = [create_signal(false), create_signal(false)];
    let branch_shown =
        showing.map(|own| create_memo(move || own.get() && outer.is_none_or(|outer| outer.get())));
    create_binding(move || {
        let visible = condition.get();
        if shown.get() == Some(visible) {
            if visible {
                untrack(&refresh);
            }
            return;
        }
        shown.set(Some(visible));
        let visible = usize::from(visible);
        let held = held.value();
        let mut held = held.borrow_mut();
        if let Some((host, scope)) = held[1 - visible].as_ref() {
            host.set_visible(false);
            // Told before it pauses, so an effect it wakes still runs.
            untrack(|| showing[1 - visible].set(false));
            scope.pause();
        }
        // Shown before it is built, so what a first build claims (a dock it joins) holds on.
        untrack(|| showing[visible].set(true));
        let built = held[visible].is_some();
        if !built && let Some(branch) = branches[visible] {
            let scope = Scope::detached().pausable();
            let shown_here = branch_shown[visible];
            let host = scope.context().run(|| {
                provide_context(KeptShown(shown_here));
                branch.call(()).mount_in(slot.clone())
            });
            held[visible] = Some((host, scope));
        }
        match held[visible].as_ref() {
            Some((host, scope)) => {
                if built && visible == 1 {
                    untrack(&refresh);
                }
                host.set_visible(true);
                scope.resume();
            }
            None => slot.fill(Vec::new()),
        }
    });
}

/// Builds `content` at once but hidden, shows `fallback` while anything it made is
/// pending, then shows the content for good. It fills a slot that takes one `Child` when
/// both build exactly one child.
#[component]
pub fn Loading<C, B, F>(
    fallback: RenderFn<Pending, F>,
    #[prop(children)] content: Render<(), B>,
) -> FormOf<C, <B as Counted>::Count, <F as Counted>::Count>
where
    C: SlotChild,
    B: Branch<C>,
    F: Branch<C>,
    <B as Counted>::Count: Or<<F as Counted>::Count>,
{
    let fallback = built(fallback);
    <<<B as Counted>::Count as Or<<F as Counted>::Count>>::Count as Count>::form(
        DynamicSegment::new(move |slot: ChildSlot<C>| {
            let scope = Scope::detached().pausable_effects();
            scope.pause();
            let (boundary, host) = scope.context().run(|| {
                let boundary = Boundary::provide();
                (
                    boundary,
                    content.call(()).into_children().mount_hidden(slot.clone()),
                )
            });
            let content = store((host, scope));
            let shown = store(RefCell::new(None::<(Pending, Scope)>));
            let revealed = StoredCell::new(false);
            create_binding(move || {
                if revealed.get() {
                    return;
                }
                let state = boundary.state().and_then(|()| {
                    content.value().1.flush_owned();
                    boundary.state()
                });
                let shown = shown.value();
                let mut shown = shown.borrow_mut();
                match state {
                    Ok(()) => {
                        revealed.set(true);
                        boundary.reveal();
                        let content = content.value();
                        content.0.set_visible(true);
                        drop(shown.take());
                        content.1.resume();
                    }
                    Err(pending) => {
                        if shown.as_ref().is_some_and(|(held, _)| *held == pending) {
                            return;
                        }
                        let built = mount(&slot, || fallback.call(pending.clone()));
                        drop(shown.replace((pending, built)));
                    }
                }
            });
        }),
    )
}

/// Builds `view` from `value`, again each time it changes to a value that differs from the
/// last (`PartialEq`), so a value set again unchanged keeps what was built. Given a `fallback`, `value` is
/// an `Option`: `view` takes what is in it, and the fallback shows while it is `None`. It
/// fills a slot that takes one `Child` when what it builds is exactly one child.
#[component]
pub fn Dynamic<V, C, B, F>(
    value: Prop<V>,
    fallback: F,
    #[prop(children)] view: RenderFn<F::Handed, B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    V: Clone + PartialEq + 'static,
    C: SlotChild,
    B: Branch<C>,
    F: ValueFallback<V, C>,
    <B as Counted>::Count: Or<F::Count>,
{
    let view = built(view);
    let fallback = fallback.into_render();
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| {
            let held = store(RefCell::new(None::<(V, Option<Scope>)>));
            create_binding(move || {
                let next = value.get();
                let held = held.value();
                let mut held = held.borrow_mut();
                if held.as_ref().is_some_and(|(last, _)| *last == next) {
                    return;
                }
                let shown = next.clone();
                let value = F::split(next);
                let built = match value {
                    Some(value) => Some(mount(&slot, || view.call(value))),
                    None => fallback.map(|fallback| mount(&slot, || fallback.call(()))),
                };
                if built.is_none() {
                    slot.fill(Vec::new());
                }
                drop(held.replace((shown, built)));
            });
        },
    ))
}

/// How a `ForEach` tells its items apart, and so what a row is handed. Without a `key`, an
/// item is its own key: a changed item is a new row, built again and handed the item. With a
/// `key`, a row stays while its item changes and is handed a signal of it.
pub trait RowKey<T>: 'static {
    type Key: Clone + Hash + Eq + 'static;
    /// What a row is handed: the item, or a signal of it.
    type Item: 'static;
    /// Whether a row is kept while its item changes, and so handed a signal of it.
    const KEEPS: bool;

    fn key(&self, item: &T) -> Self::Key;

    /// What a new row is handed for `item`, and the signal to keep it current, if it has one.
    fn hand(item: T) -> (Self::Item, Option<Signal<T>>);

    /// Gives a kept row its item again, which it sees if the item changed.
    fn refresh(signal: Signal<T>, item: T);
}

impl<T: Clone + Hash + Eq + 'static> RowKey<T> for MissingProp {
    type Key = T;
    type Item = T;
    const KEEPS: bool = false;

    fn key(&self, item: &T) -> T {
        item.clone()
    }

    fn hand(item: T) -> (T, Option<Signal<T>>) {
        (item, None)
    }

    fn refresh(_signal: Signal<T>, _item: T) {}
}

impl<T, K, F> RowKey<T> for F
where
    T: Clone + PartialEq + 'static,
    K: Clone + Hash + Eq + 'static,
    F: Fn(&T) -> K + 'static,
{
    type Key = K;
    type Item = ReadSignal<T>;
    const KEEPS: bool = true;

    fn key(&self, item: &T) -> K {
        self(item)
    }

    fn hand(item: T) -> (ReadSignal<T>, Option<Signal<T>>) {
        let signal = create_signal(item);
        (signal.read_only(), Some(signal))
    }

    fn refresh(signal: Signal<T>, item: T) {
        signal.set(item);
    }
}

/// What a row closure takes: what its `ForEach` hands it, alone, or with the row's place in
/// the list, which changes as rows move: `|item: T|` or `|item: T, index: ReadSignal<usize>|`.
#[diagnostic::on_unimplemented(
    message = "a `ForEach` row takes what the list hands it, `{I}`, and may take its index after it",
    label = "write `|item: {I}|` or `|item: {I}, index: ReadSignal<usize>|`"
)]
pub trait RowArgs<I>: 'static {
    fn row(item: I, index: ReadSignal<usize>) -> Self;
}

impl<I: 'static> RowArgs<I> for I {
    fn row(item: I, _index: ReadSignal<usize>) -> I {
        item
    }
}

impl<I: 'static> RowArgs<I> for (I, ReadSignal<usize>) {
    fn row(item: I, index: ReadSignal<usize>) -> Self {
        (item, index)
    }
}

/// A built row: one child stored as it is, or a run that fills a slot of the `Rows`.
enum Row<C: SlotChild> {
    One(C::Stored),
    Run(SlotId, Rc<NestedHost<C>>),
}

impl<C: SlotChild> Clone for Row<C> {
    fn clone(&self) -> Self {
        match self {
            Row::One(stored) => Row::One(stored.clone()),
            Row::Run(slot, host) => Row::Run(*slot, host.clone()),
        }
    }
}

/// Where a `ForEach`'s rows go: each row that is a run fills a slot of its own, and the rows
/// fill the `ForEach`'s place one after another, in the order of the items, so moving a row
/// moves its whole run.
struct Rows<C: SlotChild> {
    slot: ChildSlot<C>,
    next: Cell<u32>,
    runs: RefCell<HashMap<SlotId, Vec<C::Stored>>>,
    order: RefCell<Vec<Row<C>>>,
    building: Cell<bool>,
    /// Bumped when a row's run changes on its own, so the rows fill their place again once
    /// per flush however many rows changed.
    changed: Signal<u64>,
    /// While the `fallback` fills the place, the rows must not.
    empty: Cell<bool>,
    /// Tells the `ForEach`'s handle what each row holds once the rows fill their place.
    shown: RefCell<Option<Box<dyn Fn(Vec<Vec<C::Stored>>)>>>,
}

impl<C: SlotChild> Rows<C> {
    fn publish(&self) {
        if self.empty.get() {
            return;
        }
        let runs = self.runs.borrow();
        let order = self.order.borrow();
        let shown = self.shown.borrow();
        let Some(shown) = shown.as_ref() else {
            let mut children = Vec::with_capacity(order.len());
            for row in order.iter() {
                match row {
                    Row::One(stored) => children.push(stored.clone()),
                    Row::Run(slot, _) => children.extend(runs.get(slot).into_iter().flatten().cloned()),
                }
            }
            drop((runs, order));
            self.slot.fill(children);
            return;
        };
        let rows: Vec<Vec<C::Stored>> = order
            .iter()
            .map(|row| match row {
                Row::One(stored) => vec![stored.clone()],
                Row::Run(slot, _) => runs.get(slot).cloned().unwrap_or_default(),
            })
            .collect();
        drop((runs, order));
        self.slot.fill(rows.iter().flatten().cloned().collect());
        shown(rows);
    }

    fn build(self: &Rc<Self>, children: Children<C>) -> Row<C> {
        match children.into_only() {
            Ok(child) => Row::One(self.slot.store(child)),
            Err(children) => {
                let host: Rc<dyn SlotHost<C>> = self.clone();
                let slot = ChildSlot::in_host(host);
                let id = slot
                    .host_slot()
                    .expect("a slot opened in a host knows its id");
                Row::Run(id, children.mount_in(slot))
            }
        }
    }
}

impl<C: SlotChild> SlotHost<C> for Rows<C> {
    fn store(&self, child: C) -> C::Stored {
        self.slot.store(child)
    }

    fn append(&self, _stored: C::Stored) {
        unreachable!("a row of a `ForEach` fills a slot of its own");
    }

    fn open(&self) -> SlotId {
        let id = SlotId::nth(self.next.get());
        self.next.set(self.next.get() + 1);
        self.runs.borrow_mut().insert(id, Vec::new());
        id
    }

    fn fill(&self, slot: SlotId, items: Vec<C::Stored>) {
        if let Some(run) = self.runs.borrow_mut().get_mut(&slot) {
            *run = items;
        }
        if !self.building.get() {
            self.changed.update(|changed| *changed += 1);
        }
    }
}

/// A handle on what a `ForEach` or `VirtualList` shows: the order of its keys, each key's
/// place in it, and the children each built row holds (one on the fast path, a run for a row
/// of several). Made like a `NodeRef`, handed to one list as `handle`, and empty until it
/// builds and again once the list is gone; a key no longer shown has no place, and a key a
/// `VirtualList` has not built has a place but no nodes. `S` is what a child is known by
/// (`SlotChild::Node`): `NodeId` for node children, whatever parent they are placed in.
///
/// Looking a key up costs the same however many keys there are. A key shown more than once
/// is known by its first place (`index_of`, `index`), and `nodes_of` gives the nodes of every
/// row it has, in order.
pub struct ForEachRef<K: 'static, S: 'static> {
    shown: Stored<RefCell<Shown<K, S>>>,
    changed: Trigger,
}

/// A row a list has built, as its handle holds it: its key, the index signal the list handed
/// it, and the children it holds.
pub struct ShownRow<K, S> {
    pub key: K,
    pub index: ReadSignal<usize>,
    pub nodes: Vec<S>,
}

struct Shown<K, S> {
    keys: Rc<[K]>,
    /// Each key's places in `keys`, made when first asked for after the keys change.
    places: Box<dyn Lookup<K>>,
    places_made: bool,
    /// The built rows, in the order shown.
    rows: Vec<ShownRow<K, S>>,
    /// Each key's positions in `rows`.
    row_of: Box<dyn Lookup<K>>,
    /// Which claim holds the handle: each list's `claim` takes it over, so a list rebuilt
    /// under the same handle (a `Dynamic` builds the new branch before dropping the old)
    /// replaces the old one, whose later fills and cleanup leave the handle alone.
    owner: u64,
}

/// Finds each key's positions in a sequence of keys. Made where the key can be hashed, so a
/// handle's own bounds need only `PartialEq`.
trait Lookup<K> {
    fn fill(&mut self, keys: &mut dyn Iterator<Item = &K>);
    fn first(&self, key: &K) -> Option<usize>;
    fn each(&self, key: &K, found: &mut dyn FnMut(usize));
}

/// A key's first position and any later ones.
type Positions = (usize, Vec<usize>);

impl<K: Clone + Eq + Hash> Lookup<K> for HashMap<K, Positions> {
    fn fill(&mut self, keys: &mut dyn Iterator<Item = &K>) {
        self.clear();
        for (at, key) in keys.enumerate() {
            match self.get_mut(key) {
                Some((_, later)) => later.push(at),
                None => {
                    self.insert(key.clone(), (at, Vec::new()));
                }
            }
        }
    }

    fn first(&self, key: &K) -> Option<usize> {
        self.get(key).map(|(first, _)| *first)
    }

    fn each(&self, key: &K, found: &mut dyn FnMut(usize)) {
        if let Some((first, later)) = self.get(key) {
            found(*first);
            later.iter().for_each(|at| found(*at));
        }
    }
}

impl<K, S> Clone for ForEachRef<K, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, S> Copy for ForEachRef<K, S> {}

impl<K: Clone + Eq + Hash + 'static, S: Clone + 'static> Default for ForEachRef<K, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Eq + Hash + 'static, S: Clone + 'static> ForEachRef<K, S> {
    /// Panics with no current scope, like `store`.
    pub fn new() -> Self {
        Self {
            shown: store(RefCell::new(Shown {
                keys: Rc::from([]),
                places: Box::new(HashMap::<K, Positions>::default()),
                places_made: true,
                rows: Vec::new(),
                row_of: Box::new(HashMap::<K, Positions>::default()),
                owner: 0,
            })),
            changed: create_trigger(),
        }
    }
}

impl<K: Clone + PartialEq + 'static, S: Clone + 'static> ForEachRef<K, S> {
    /// The keys shown, in order; tracked, so a computation reading it reruns when the order
    /// or what a row holds changes.
    pub fn keys(&self) -> Rc<[K]> {
        self.changed.track();
        self.with_shown(|shown| Rc::clone(&shown.keys))
            .unwrap_or_else(|| Rc::from([]))
    }

    /// Where `key` sits now (its first place, if it repeats), untracked.
    pub fn index_of(&self, key: &K) -> Option<usize> {
        let shown = self.shown.try_value()?;
        let mut shown = shown.borrow_mut();
        if !shown.places_made {
            let Shown { keys, places, .. } = &mut *shown;
            places.fill(&mut keys.iter());
            shown.places_made = true;
        }
        shown.places.first(key)
    }

    /// Where `key`'s built row sits (its first, if it repeats), as the signal the row was
    /// handed.
    pub fn index(&self, key: &K) -> Option<ReadSignal<usize>> {
        self.with_shown(|shown| {
            let at = shown.row_of.first(key)?;
            Some(shown.rows[at].index)
        })
        .flatten()
    }

    /// The nodes `key`'s rows hold, every repeat's in order; none for a key not shown or not
    /// built.
    pub fn nodes_of(&self, key: &K) -> Vec<S> {
        self.with_shown(|shown| {
            let mut nodes = Vec::new();
            shown
                .row_of
                .each(key, &mut |at| nodes.extend(shown.rows[at].nodes.iter().cloned()));
            nodes
        })
        .unwrap_or_default()
    }

    /// The key whose built row holds `node`; looks through the built rows.
    pub fn key_of(&self, node: &S) -> Option<K>
    where
        S: PartialEq,
    {
        self.with_shown(|shown| {
            shown
                .rows
                .iter()
                .find(|row| row.nodes.contains(node))
                .map(|row| row.key.clone())
        })
        .flatten()
    }

    /// Visits each built row in the order shown, untracked: as many as the list built, not
    /// as many keys as it has.
    pub fn for_each_row(&self, mut visit: impl FnMut(&ShownRow<K, S>)) {
        self.with_shown(|shown| shown.rows.iter().for_each(&mut visit));
    }

    /// For the list that fills this handle, in its scope: it takes the handle over from any
    /// list before it and holds it until its scope ends, when the handle is empty again
    /// unless another list has taken it since. A list filling a handle another has taken
    /// since is two lists sharing one, a mistake.
    pub fn claim(&self) -> ForEachFill<K, S> {
        let owner = self
            .shown
            .try_value()
            .map_or(0, |shown| {
                let mut shown = shown.borrow_mut();
                shown.owner += 1;
                shown.owner
            });
        let handle = *self;
        beui_reactive::on_cleanup(move || {
            let current = handle
                .shown
                .try_value()
                .is_some_and(|shown| shown.borrow().owner == owner);
            if current {
                handle.set_keys(owner, Rc::from([]));
                handle.set_rows(owner, Vec::new());
            }
        });
        ForEachFill(handle, owner)
    }

    /// Whether `owner` still holds the handle; a fill from one that does not is two lists
    /// sharing it.
    fn holds(&self, shown: &Shown<K, S>, owner: u64) -> bool {
        debug_assert!(shown.owner == owner, "two lists share one ForEachRef");
        shown.owner == owner
    }

    fn set_keys(&self, owner: u64, keys: Rc<[K]>) {
        if let Some(shown) = self.shown.try_value() {
            let mut shown = shown.borrow_mut();
            if !self.holds(&shown, owner) {
                return;
            }
            shown.keys = keys;
            shown.places_made = false;
            drop(shown);
            self.changed.notify();
        }
    }

    fn set_rows(&self, owner: u64, rows: Vec<ShownRow<K, S>>) {
        if let Some(shown) = self.shown.try_value() {
            let mut shown = shown.borrow_mut();
            if !self.holds(&shown, owner) {
                return;
            }
            let Shown { rows: held, row_of, .. } = &mut *shown;
            *held = rows;
            row_of.fill(&mut held.iter().map(|row| &row.key));
            drop(shown);
            self.changed.notify();
        }
    }

    fn with_shown<R>(&self, read: impl FnOnce(&Shown<K, S>) -> R) -> Option<R> {
        let shown = self.shown.try_value()?;
        let shown = shown.borrow();
        Some(read(&shown))
    }
}

/// How the list that claimed a `ForEachRef` fills it.
pub struct ForEachFill<K: 'static, S: 'static>(ForEachRef<K, S>, u64);

impl<K, S> Clone for ForEachFill<K, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, S> Copy for ForEachFill<K, S> {}

impl<K: Clone + PartialEq + 'static, S: Clone + 'static> ForEachFill<K, S> {
    /// The keys shown, in order; the rows built may follow later.
    pub fn keys(&self, keys: Rc<[K]>) {
        self.0.set_keys(self.1, keys);
    }

    /// The rows built, in the order shown.
    pub fn rows(&self, rows: Vec<ShownRow<K, S>>) {
        self.0.set_rows(self.1, rows);
    }
}

struct BuiltRow<C: SlotChild, T> {
    row: Row<C>,
    item: Option<Signal<T>>,
    index: Signal<usize>,
}

impl<C: SlotChild, T> Clone for BuiltRow<C, T> {
    fn clone(&self) -> Self {
        Self {
            row: self.row.clone(),
            item: self.item,
            index: self.index,
        }
    }
}

/// Builds `view` once for each item of `value` and keeps it while the item stays, in the
/// order of `value`. Without a `key` an item is its own key, and a row is handed the item;
/// with one a row is handed a signal of its item, which changes in place. A row may take
/// its `index` too. A row builds any number of children. Given a `fallback`, it shows while
/// `value` is empty. Given a `handle`, it tells it what it shows (`ForEachRef`).
#[component]
pub fn ForEach<T, KF, H, C, R, F>(
    value: Prop<Vec<T>>,
    key: KF,
    fallback: F,
    #[prop(default = None)] handle: Option<ForEachRef<KF::Key, C::Node>>,
    #[prop(children)] view: RenderFn<H, R>,
) -> DynamicSegment<C>
where
    T: Clone + 'static,
    KF: RowKey<T>,
    H: RowArgs<KF::Item>,
    C: SlotChild,
    R: Branch<C>,
    F: Fallback<C>,
{
    let fallback = fallback.into_render();
    let key = Rc::new(key);
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let rows = Rc::new(Rows {
            slot: slot.clone(),
            next: Cell::new(0),
            runs: RefCell::default(),
            order: RefCell::default(),
            building: Cell::new(false),
            changed: create_signal(0),
            empty: Cell::new(false),
            shown: RefCell::new(None),
        });
        let shown_keys = Rc::new(RefCell::new(Vec::<(KF::Key, ReadSignal<usize>)>::new()));
        let fill = handle.map(|handle| handle.claim());
        if let Some(fill) = fill {
            let shown_keys = shown_keys.clone();
            *rows.shown.borrow_mut() = Some(Box::new(move |children: Vec<Vec<C::Stored>>| {
                let keys = shown_keys.borrow();
                fill.rows(
                    keys.iter()
                        .zip(children)
                        .map(|((key, index), children)| ShownRow {
                            key: key.clone(),
                            index: *index,
                            nodes: children.iter().map(C::node).collect(),
                        })
                        .collect(),
                );
            }));
        }
        {
            let rows = rows.clone();
            create_binding(move || {
                rows.changed.get();
                untrack(|| rows.publish());
            });
        }
        let pending = Rc::new(RefCell::new(
            HashMap::<(KF::Key, usize), (T, usize)>::default(),
        ));
        let built = {
            let rows = rows.clone();
            let pending = pending.clone();
            store(KeyedItems::numbered(move |at: KF::Key, repeat: usize| {
                let (item, index) = pending
                    .borrow_mut()
                    .remove(&(at, repeat))
                    .expect("a row is built for an item of this pass");
                let (handed, item) = KF::hand(item);
                let index = create_signal(index);
                let children = view.call(H::row(handed, index.read_only())).into_children();
                BuiltRow {
                    row: rows.build(children),
                    item,
                    index,
                }
            }))
        };
        let empty = store(RefCell::new(None::<Scope>));
        create_binding(move || {
            let items = value.get();
            if items.is_empty()
                && let Some(fallback) = fallback
            {
                rows.empty.set(true);
                let empty = empty.value();
                let mut empty = empty.borrow_mut();
                if empty.is_none() {
                    *empty = Some(mount(&slot, || fallback.call(())));
                }
                drop(built.value().map(Vec::new()));
                rows.order.borrow_mut().clear();
                rows.runs.borrow_mut().clear();
                shown_keys.borrow_mut().clear();
                if let Some(fill) = fill {
                    fill.keys(Rc::from([]));
                    fill.rows(Vec::new());
                }
                return;
            }
            drop(empty.value().borrow_mut().take());
            rows.empty.set(false);
            let mut keys = Vec::with_capacity(items.len());
            let mut changed = Vec::new();
            {
                let mut pending = pending.borrow_mut();
                pending.clear();
                let mut repeats = HashMap::<KF::Key, usize>::default();
                for (index, item) in items.into_iter().enumerate() {
                    let at = key.key(&item);
                    let repeat = repeats.entry(at.clone()).or_default();
                    if KF::KEEPS {
                        changed.push(Some(item.clone()));
                    } else {
                        changed.push(None);
                    }
                    pending.insert((at.clone(), *repeat), (item, index));
                    *repeat += 1;
                    keys.push(at);
                }
            }
            rows.building.set(true);
            if let Some(fill) = fill {
                fill.keys(Rc::from(keys.as_slice()));
            }
            let order = keys.clone();
            built.value().map(keys).commit(|kept, gone| {
                let mut runs = rows.runs.borrow_mut();
                for gone in gone {
                    if let Row::Run(slot, _) = gone.row {
                        runs.remove(&slot);
                    }
                }
                drop(runs);
                for (index, (built, item)) in kept.iter().zip(changed).enumerate() {
                    if built.index.get_untracked() != index {
                        built.index.set(index);
                    }
                    if let (Some(signal), Some(item)) = (built.item, item) {
                        KF::refresh(signal, item);
                    }
                }
                *shown_keys.borrow_mut() = order
                    .into_iter()
                    .zip(&kept)
                    .map(|(key, built)| (key, built.index.read_only()))
                    .collect();
                *rows.order.borrow_mut() = kept.into_iter().map(|built| built.row).collect();
            });
            pending.borrow_mut().clear();
            rows.building.set(false);
            rows.publish();
        });
    })
}

/// What a `Case` tests the value of its `Match` with, beside `is`: a closure over the value,
/// or nothing.
pub trait CaseWhen<V>: 'static {
    fn holds(&self, value: &V) -> bool;
}

impl<V> CaseWhen<V> for MissingProp {
    fn holds(&self, _value: &V) -> bool {
        true
    }
}

impl<V, W: Fn(&V) -> bool + 'static> CaseWhen<V> for W {
    fn holds(&self, value: &V) -> bool {
        self(value)
    }
}

/// What a `Match` matches: its value, named through this so that the value's type comes
/// from `value` alone (a signal, memo or closure), never from what a `Case` compares with.
pub trait Matched {
    type Value;
}

impl<V> Matched for V {
    type Value = V;
}

/// What a `Case` compares the value of its `Match` with: the value itself, or nothing.
pub trait CaseIs<V>: 'static {
    fn holds(&self, value: &V) -> bool;
}

impl<V> CaseIs<V> for MissingProp {
    fn holds(&self, _value: &V) -> bool {
        true
    }
}

impl<V: PartialEq + 'static> CaseIs<V> for V {
    fn holds(&self, value: &V) -> bool {
        self == value
    }
}

/// One case of a `Match`, made by `Case`.
pub struct CaseOf<V: 'static, C: SlotChild> {
    test: Box<dyn Fn(&V) -> bool>,
    view: RenderFn<ReadSignal<V>, Children<C>>,
}

crate::value_child_type!(<V, C: SlotChild> CaseOf<V, C>);

/// A case of a `Match`: it holds when the value equals `is` and `when` holds of it (either
/// may be left out). Its children are built when it becomes the case that holds, and are
/// handed a signal of the value, which changes in place while it stays that case.
#[component]
pub fn Case<V, C, B, I, W>(
    is: I,
    when: W,
    #[prop(children)] view: RenderFn<ReadSignal<V>, B>,
) -> CaseOf<V, C>
where
    V: 'static,
    C: SlotChild,
    B: Branch<C>,
    I: CaseIs<V>,
    W: CaseWhen<V>,
{
    CaseOf {
        test: Box::new(move |value| is.holds(value) && when.holds(value)),
        view: built(view),
    }
}

/// Shows the first `Case` that holds for `value`, and `fallback`, if given, while none does.
/// A case is built when it becomes the one that holds and kept while it stays so, handed a
/// signal of the value that changes in place:
/// `<Match value={shape}><Case is={Shape::Circle}>..</Case><Case when={|s: &Shape| s.is_round()}>|shape: ReadSignal<Shape>| ..</Case></Match>`.
#[component]
pub fn Match<V, C, F>(
    value: Prop<V>,
    fallback: F,
    #[prop(children)] cases: Children<CaseOf<<V as Matched>::Value, C>>,
) -> DynamicSegment<C>
where
    V: Matched<Value = V> + Clone + PartialEq + 'static,
    C: SlotChild,
    F: Fallback<C>,
{
    let fallback = fallback.into_render();
    let cases = cases.into_run();
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held = store(RefCell::new(
            None::<(Option<usize>, Option<Signal<V>>, Option<Scope>)>,
        ));
        let cases = cases.clone();
        create_binding(move || {
            let next = value.get();
            let found = cases
                .items()
                .into_iter()
                .enumerate()
                .find(|(_, case)| (case.test)(&next));
            let index = found.as_ref().map(|(index, _)| *index);
            let held = held.value();
            let mut held = held.borrow_mut();
            if let Some((shown, signal, _)) = held.as_ref()
                && *shown == index
            {
                if let Some(signal) = signal {
                    signal.set(next);
                }
                return;
            }
            let (signal, built) = match found {
                Some((_, case)) => {
                    let signal = Cell::new(None::<Signal<V>>);
                    let built = mount(&slot, || {
                        let made = create_signal(next);
                        signal.set(Some(made));
                        case.view.call(made.read_only())
                    });
                    (signal.get(), Some(built))
                }
                None => (
                    None,
                    fallback.map(|fallback| mount(&slot, || fallback.call(()))),
                ),
            };
            if built.is_none() {
                slot.fill(Vec::new());
            }
            drop(held.replace((index, signal, built)));
        });
    })
}

/// Builds `view` once for each `key` of `value`, handing it the value as a signal, so a
/// change that keeps the key updates in place. Given a `fallback`, `value` is an `Option`
/// and the fallback shows while it is `None`.
#[component]
pub fn Keyed<V, K, KF, C, B, F>(
    value: Prop<V>,
    fallback: F,
    key: KF,
    #[prop(children)] view: RenderFn<ReadSignal<F::Handed>, B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    V: Clone + 'static,
    K: PartialEq + 'static,
    KF: Fn(&F::Handed) -> K + 'static,
    C: SlotChild,
    B: Branch<C>,
    F: ValueFallback<V, C>,
    F::Handed: PartialEq,
    <B as Counted>::Count: Or<F::Count>,
{
    let view = built(view);
    let fallback = fallback.into_render();
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| {
            let current = store(RefCell::new(None::<Signal<F::Handed>>));
            let held = store(RefCell::new(None::<(Option<K>, Option<Scope>)>));
            create_binding(move || {
                let value = F::split(value.get());
                let next = value.as_ref().map(&key);
                let held = held.value();
                let mut held = held.borrow_mut();
                if held.as_ref().is_some_and(|(key, _)| *key == next) {
                    if let (Some(value), Some(signal)) = (value, *current.value().borrow()) {
                        signal.set(value);
                    }
                    return;
                }
                let built = match value {
                    Some(value) => Some(mount(&slot, move || {
                        let signal = create_signal(value);
                        *current.value().borrow_mut() = Some(signal);
                        view.call(signal.read_only())
                    })),
                    None => {
                        *current.value().borrow_mut() = None;
                        fallback.map(|fallback| mount(&slot, || fallback.call(())))
                    }
                };
                if built.is_none() {
                    slot.fill(Vec::new());
                }
                drop(held.replace((next, built)));
            });
        },
    ))
}
