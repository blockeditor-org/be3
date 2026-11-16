use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use beui_reactive::{
    ReadSignal, Scope, ScopeContext, Set, Signal, Stored, Trigger, create_binding, create_signal,
    create_trigger, store,
};

use crate::child_list::{ChildList, SlotId};
use crate::component::{BuildFn, ChildValue, ComponentContext, MissingProp};
use crate::prop::{IntoProp, Prop};

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is not something a parent can keep a run of",
    label = "say how a run of these is kept with `child_type!` or `value_child_type!`"
)]
pub trait SlotChild: Sized + 'static {
    type Stored: Clone + 'static;
    /// What a kept child is known by to a list's handle (`ForEachRef`): its node, for node
    /// children of any parent kind, so one handle type serves them all.
    type Node: Clone + 'static;

    fn store(self) -> Self::Stored;

    fn node(stored: &Self::Stored) -> Self::Node;
}

pub trait SlotHost<C: SlotChild> {
    fn store(&self, child: C) -> C::Stored {
        child.store()
    }

    fn append(&self, stored: C::Stored);

    fn open(&self) -> SlotId;

    fn fill(&self, slot: SlotId, items: Vec<C::Stored>);
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot keep `{C}` children",
    label = "implement `SlotHost<{C}>` for what keeps these children"
)]
pub trait IntoSlotHost<C: SlotChild> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<C>>;
}

impl<C: SlotChild> IntoSlotHost<C> for Rc<dyn SlotHost<C>> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<C>> {
        self
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be a child of this component",
    label = "this component takes `{T}` children"
)]
pub trait IntoChild<T> {
    fn into_child(self) -> T;
}

pub fn into_child<T>(child: impl IntoChild<T>) -> T {
    child.into_child()
}

pub enum ChildSegment<T: SlotChild> {
    One(T),
    Many(Vec<T>),
    Nested(Vec<ChildSegment<T>>),
    Dynamic(DynamicSegment<T>),
}

impl<T: SlotChild> ChildSegment<T> {
    fn map(self, change: &Rc<dyn Fn(T) -> T>) -> Self {
        match self {
            ChildSegment::One(item) => ChildSegment::One(change(item)),
            ChildSegment::Many(items) => {
                ChildSegment::Many(items.into_iter().map(|item| change(item)).collect())
            }
            ChildSegment::Nested(segments) => ChildSegment::Nested(
                segments
                    .into_iter()
                    .map(|segment| segment.map(change))
                    .collect(),
            ),
            ChildSegment::Dynamic(segment) => ChildSegment::Dynamic(segment.map(change.clone())),
        }
    }
}

/// How many children a built value fills its place with, as a type: `CountOne`,
/// `CountZeroOrOne` (a `Show` without a `fallback`), `CountMany` (a run), or `CountZero` (a
/// `fallback` that was not given). It travels as `IntoSegment::Count`, and decides which form
/// control flow returns (`Form`).
pub trait Count: 'static {
    /// The lazy form control flow of this count returns: `OneOf`, `ZeroOrOne` or a run
    /// (`DynamicSegment`).
    type Form<C: SlotChild>: IntoSegment<C> + ChildValue;

    fn form<C: SlotChild>(segment: DynamicSegment<C>) -> Self::Form<C>;
}

pub struct CountOne;
pub struct CountZeroOrOne;
pub struct CountMany;
pub struct CountZero;
/// No branch at all, so it adds nothing to the other: a `Dynamic` or `Keyed` without a
/// `fallback` always builds its render.
pub struct CountNone;

macro_rules! count {
    ($count:ident, $form:ident, $make:expr) => {
        impl Count for $count {
            type Form<C: SlotChild> = $form<C>;

            fn form<C: SlotChild>(segment: DynamicSegment<C>) -> $form<C> {
                $make(segment)
            }
        }
    };
}

count!(CountOne, OneOf, OneOf);
count!(CountZeroOrOne, ZeroOrOne, ZeroOrOne);
count!(CountZero, ZeroOrOne, ZeroOrOne);
count!(CountNone, ZeroOrOne, ZeroOrOne);
count!(CountMany, DynamicSegment, std::convert::identity);

/// The count of a place that holds either of two branches.
pub trait Or<Other: Count>: Count {
    type Count: Count;
}

macro_rules! or_counts {
    ($($a:ident | $b:ident = $c:ident;)*) => {
        $(impl Or<$b> for $a {
            type Count = $c;
        })*
    };
}

or_counts! {
    CountOne | CountOne = CountOne;
    CountOne | CountZeroOrOne = CountZeroOrOne;
    CountOne | CountZero = CountZeroOrOne;
    CountOne | CountMany = CountMany;
    CountZeroOrOne | CountOne = CountZeroOrOne;
    CountZeroOrOne | CountZeroOrOne = CountZeroOrOne;
    CountZeroOrOne | CountZero = CountZeroOrOne;
    CountZeroOrOne | CountMany = CountMany;
    CountZero | CountOne = CountZeroOrOne;
    CountZero | CountZeroOrOne = CountZeroOrOne;
    CountZero | CountZero = CountZero;
    CountZero | CountMany = CountMany;
    CountMany | CountOne = CountMany;
    CountMany | CountZeroOrOne = CountMany;
    CountMany | CountZero = CountMany;
    CountMany | CountMany = CountMany;
    CountOne | CountNone = CountOne;
    CountZeroOrOne | CountNone = CountZeroOrOne;
    CountZero | CountNone = CountZero;
    CountMany | CountNone = CountMany;
}

/// The form control flow over branches `B` and `F` returns, for a parent whose children are
/// `C`s.
pub type FormOf<C, B, F> = <<B as Or<F>>::Count as Count>::Form<C>;

/// How many children a built value fills its place with (`Count`), whatever its parent:
/// the count has to be known before the parent is, since it picks the form a `Show` returns
/// and the form is how the parent is found.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` is not something a branch of a `Show`, `Dynamic` or `Keyed` can build",
    label = "build children here: tags, a `view!`, or a component's value"
)]
pub trait Counted {
    type Count: Count;
}

macro_rules! counted {
    ($count:ident: $(<$($generic:ident),*> $ty:ty),* $(,)?) => {
        $(impl<$($generic: 'static),*> Counted for $ty {
            type Count = $count;
        })*
    };
}

counted!(CountOne:
    <> &'static str,
    <> String,
    <T> Prop<T>,
    <T> beui_reactive::Memo<T>,
    <T> beui_reactive::ReadSignal<T>,
    <T> Signal<T>,
);

impl<T: SlotChild> Counted for OneOf<T> {
    type Count = CountOne;
}

impl<T: SlotChild> Counted for ZeroOrOne<T> {
    type Count = CountZeroOrOne;
}

impl<T: SlotChild> Counted for DynamicSegment<T> {
    type Count = CountMany;
}

impl<T: SlotChild> Counted for Children<T> {
    type Count = CountMany;
}

impl<T: SlotChild, const N: usize> Counted for [ChildSegment<T>; N] {
    type Count = CountMany;
}

impl<H: Counted> Counted for SingleChild<H> {
    type Count = H::Count;
}

impl<H: Counted<Count = CountOne>> Counted for Option<H> {
    type Count = CountZeroOrOne;
}

/// What a branch of `Show`, `Dynamic`, `Keyed` or `Loading` builds: one child or a run of
/// them, for a parent whose children are `C`s. Its count decides the control's form.
pub trait Branch<C: SlotChild>: IntoSegment<C> + Counted + 'static {
    fn into_children(self) -> Children<C>;
}

impl<C: SlotChild, B: IntoSegment<C> + Counted + 'static> Branch<C> for B {
    fn into_children(self) -> Children<C> {
        Children::from(self.into_segment())
    }
}

/// The `fallback` of a `Show`: a closure building what shows while its condition is false,
/// or nothing when none is given.
#[diagnostic::on_unimplemented(
    message = "a `fallback` is a closure that builds children: `fallback={{|| view! {{ ... }}}}`",
    label = "write a closure here"
)]
pub trait Fallback<C: SlotChild> {
    type Count: Count;

    fn into_render(self) -> Option<BuildFn<Children<C>>>;
}

impl<C: SlotChild> Fallback<C> for MissingProp {
    type Count = CountZero;

    fn into_render(self) -> Option<BuildFn<Children<C>>> {
        None
    }
}

impl<C: SlotChild, F: Fn() -> B + 'static, B: Branch<C>> Fallback<C> for F {
    type Count = B::Count;

    fn into_render(self) -> Option<BuildFn<Children<C>>> {
        Some(BuildFn::new(move || self().into_children()))
    }
}

/// The `fallback` of a `Dynamic` or `Keyed`: given one, the value is an `Option` and the
/// fallback builds while it is `None`; without one, the render takes the value as it is.
#[diagnostic::on_unimplemented(
    message = "a `fallback` on a `Dynamic` or `Keyed` needs an `Option` value, and is a closure that builds children, or `()` for nothing",
    label = "give `value` an `Option<_>`, or leave the fallback out"
)]
pub trait ValueFallback<V, C: SlotChild> {
    type Handed: Clone + 'static;
    type Count: Count;

    fn split(value: V) -> Option<Self::Handed>;

    fn into_render(self) -> Option<BuildFn<Children<C>>>;
}

impl<V: Clone + 'static, C: SlotChild> ValueFallback<V, C> for MissingProp {
    type Handed = V;
    type Count = CountNone;

    fn split(value: V) -> Option<V> {
        Some(value)
    }

    fn into_render(self) -> Option<BuildFn<Children<C>>> {
        None
    }
}

/// `fallback=()`: the value is an `Option`, and nothing shows while it is `None`.
impl<T: Clone + 'static, C: SlotChild> ValueFallback<Option<T>, C> for () {
    type Handed = T;
    type Count = CountZero;

    fn split(value: Option<T>) -> Option<T> {
        value
    }

    fn into_render(self) -> Option<BuildFn<Children<C>>> {
        None
    }
}

impl<T: Clone + 'static, C: SlotChild, F: Fn() -> B + 'static, B: Branch<C>>
    ValueFallback<Option<T>, C> for F
{
    type Handed = T;
    type Count = B::Count;

    fn split(value: Option<T>) -> Option<T> {
        value
    }

    fn into_render(self) -> Option<BuildFn<Children<C>>> {
        Some(BuildFn::new(move || self().into_children()))
    }
}

/// A run of children that a `Show`, `Dynamic`, `Keyed` or `ForEach` fills in later, and
/// changes. It fills a place among its parent's children with no node of its own; a slot
/// that takes one node does not take it (`FixedChild`).
pub struct DynamicSegment<T: SlotChild> {
    scope: Option<ScopeContext>,
    install: Box<dyn FnOnce(ChildSlot<T>)>,
    child: PhantomData<T>,
}

impl<T: SlotChild> DynamicSegment<T> {
    pub fn new(install: impl FnOnce(ChildSlot<T>) + 'static) -> Self {
        Self {
            scope: None,
            install: Box::new(install),
            child: PhantomData,
        }
    }

    fn map(self, change: Rc<dyn Fn(T) -> T>) -> Self {
        let DynamicSegment { scope, install, .. } = self;
        Self {
            scope,
            install: Box::new(move |slot: ChildSlot<T>| install(slot.mapping(change))),
            child: PhantomData,
        }
    }

    fn mount(self, slot: ChildSlot<T>) {
        let DynamicSegment { scope, install, .. } = self;
        match scope {
            Some(scope) => scope.run(move || install(slot)),
            None => install(slot),
        }
    }
}

impl<T: SlotChild> ChildValue for DynamicSegment<T> {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        assert!(
            !component.has_extensions(),
            "component `{}` returns a run of children, so the bindings its body made \
             (such as `component_placed` or `component_state`) have no node to attach to",
            component.name(),
        );
        // A run handed on through components that wrap it mounts in the innermost one's scope,
        // which sees the context every one of them provides.
        if self.scope.is_none() {
            self.scope = Some(scope.clone());
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "a run of children cannot fill a slot that holds one `{T}`: put it in a `<List>` or `<Layers>` to say how the run is laid out, or give each branch exactly one root",
    label = "this builds a run of children"
)]
pub trait FixedChild<T> {
    fn into_fixed_child(self) -> T;
}

impl<T: SlotChild> IntoChild<T> for DynamicSegment<T>
where
    Self: FixedChild<T>,
{
    fn into_child(self) -> T {
        self.into_fixed_child()
    }
}

impl<T: SlotChild> IntoSegment<T> for DynamicSegment<T> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Dynamic(self)
    }
}

impl<T: SlotChild> IntoSegment<T> for Option<DynamicSegment<T>> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Nested(self.map(IntoSegment::into_segment).into_iter().collect())
    }
}

/// Exactly one child that control flow swaps: a `Show` with a `fallback` whose branches each
/// build one, a `Dynamic` or a `Keyed` building one. Among a parent's run it adds no node; in
/// a slot that takes one node it is held by a host node, a bare frame that sizes to its child
/// (`LazyChild`), which also takes `@test_id` and `@node_ref`.
pub struct OneOf<T: SlotChild>(DynamicSegment<T>);

/// At most one child that control flow swaps: a `Show` without a `fallback`. It fits a run,
/// and a slot that takes an optional node (`Option<Child>`) in a host node.
pub struct ZeroOrOne<T: SlotChild>(DynamicSegment<T>);

macro_rules! lazy_form {
    ($form:ident, $count:ident) => {
        impl<T: SlotChild> $form<T> {
            /// The child as a lazy child for a host node to hold.
            pub fn into_lazy(self) -> LazyChild<T> {
                LazyChild(self.0)
            }
        }

        impl<T: SlotChild> ChildValue for $form<T> {
            fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
                assert!(
                    !component.has_extensions(),
                    "component `{}` returns control flow, so the bindings its body made (such \
                     as `component_placed` or `component_state`) need a node: declare it \
                     `-> NodeId` and its control flow is held by a host node",
                    component.name(),
                );
                self.0.scope = Some(scope.clone());
            }
        }

        impl<T: SlotChild> IntoSegment<T> for $form<T> {
            fn into_segment(self) -> ChildSegment<T> {
                ChildSegment::Dynamic(self.0)
            }
        }

        /// Control flow built only when a value fixed when the view is built says so
        /// (`labelled.then(|| view! { <Show ..> })`) fills a run like any `Option` of a child.
        impl<T: SlotChild> IntoSegment<T> for Option<$form<T>> {
            fn into_segment(self) -> ChildSegment<T> {
                ChildSegment::Nested(self.map(IntoSegment::into_segment).into_iter().collect())
            }
        }
    };
}

lazy_form!(OneOf, CountOne);
lazy_form!(ZeroOrOne, CountZeroOrOne);

#[diagnostic::on_unimplemented(
    message = "this slot always holds one `{T}`, and this control flow can build none (a `Show` without a fallback, or `fallback=()`): add `fallback=…` so the slot always holds a child",
    label = "this can build no child"
)]
pub trait NeedsFallback<T> {
    fn into_fallback_child(self) -> T;
}

/// The nodes a core holds in a host node (`NodeId`): in a slot that takes one of them, a
/// `Show` without a `fallback` is reported as needing one.
pub trait HostedNode: SlotChild {}

impl<T: HostedNode> IntoChild<T> for ZeroOrOne<T>
where
    Self: NeedsFallback<T>,
{
    fn into_child(self) -> T {
        self.into_fallback_child()
    }
}

/// The child a host node holds and swaps (`OneOf` or `ZeroOrOne`): what a core's host
/// mounts.
pub struct LazyChild<T: SlotChild>(DynamicSegment<T>);

impl<T: SlotChild> LazyChild<T> {
    /// Puts the child in place now, and again whenever it is swapped: `set` is called with
    /// the stored child, or `None` while there is none.
    pub fn mount(self, set: impl Fn(Option<T::Stored>) + 'static) {
        let host: Rc<dyn SlotHost<T>> = Rc::new(SingleHost { set: Box::new(set) });
        self.0.mount(ChildSlot::in_host(host));
    }

    /// Mounts the child into a host that keeps a run, as its only child.
    pub fn mount_in(self, host: impl IntoSlotHost<T>) {
        Children::from(ChildSegment::Dynamic(self.0)).mount(host);
    }
}

struct SingleHost<T: SlotChild> {
    set: Box<dyn Fn(Option<T::Stored>)>,
}

impl<T: SlotChild> SlotHost<T> for SingleHost<T> {
    fn append(&self, stored: T::Stored) {
        (self.set)(Some(stored));
    }

    fn open(&self) -> SlotId {
        SlotId::FIRST
    }

    fn fill(&self, _slot: SlotId, items: Vec<T::Stored>) {
        debug_assert!(
            items.len() <= 1,
            "a host node that holds one child was filled with {}",
            items.len()
        );
        (self.set)(items.into_iter().next());
    }
}

/// What a `#[component]` body becomes to be the type the component declares: itself, or
/// for one declared `-> NodeId`, control flow held by a host node.
#[diagnostic::on_unimplemented(
    message = "this component's body builds a `{Self}`, and it is declared to return a `{T}`",
    label = "change the declared return type, or what the body builds"
)]
pub trait ComponentOutput<T> {
    fn into_output(self) -> T;
}

impl<T> ComponentOutput<T> for T {
    fn into_output(self) -> T {
        self
    }
}

impl<T: SlotChild> ComponentOutput<DynamicSegment<T>> for OneOf<T> {
    fn into_output(self) -> DynamicSegment<T> {
        self.0
    }
}

impl<T: SlotChild> ComponentOutput<DynamicSegment<T>> for ZeroOrOne<T> {
    fn into_output(self) -> DynamicSegment<T> {
        self.0
    }
}

struct RunState<S> {
    items: RefCell<ChildList<S>>,
    filled: Trigger,
}

impl<S> RunState<S> {
    fn new() -> Self {
        Self {
            items: RefCell::new(ChildList::default()),
            filled: create_trigger(),
        }
    }

    fn changed(&self) {
        self.filled.notify();
    }
}

/// The children of a slot: those written now and those a `Show`, `Dynamic` or `ForEach`
/// inside it builds later. `items` subscribes; `build` turns them into children of the
/// component's own and keeps the two in step. Cheap to clone, and fills a `Children<C>`
/// slot again, so one run of options reaches two controls (`styled::Tabs`).
pub struct Run<C: SlotChild> {
    state: Rc<RunState<C::Stored>>,
}

impl<C: SlotChild> Clone for Run<C> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<C: SlotChild> Default for Run<C> {
    fn default() -> Self {
        Self {
            state: Rc::new(RunState::new()),
        }
    }
}

impl<C: SlotChild> Run<C> {
    #[track_caller]
    pub fn items(&self) -> Vec<C::Stored> {
        self.state.filled.track();
        self.get_untracked()
    }

    pub fn get_untracked(&self) -> Vec<C::Stored> {
        self.state.items.borrow().iter().cloned().collect()
    }

    #[track_caller]
    pub fn len(&self) -> usize {
        self.state.filled.track();
        self.state.items.borrow().len()
    }

    #[track_caller]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

type HeldEach<T, R> = Stored<RefCell<Vec<(Rc<T>, <R as SlotChild>::Stored, Scope)>>>;

impl<T: 'static, C: SlotChild<Stored = Rc<T>>> Run<C> {
    pub fn build_each<R: SlotChild, U: IntoChild<R>, S: 'static>(
        &self,
        states: Stored<RefCell<Vec<S>>>,
        state: impl Fn(usize, Rc<T>) -> S + 'static,
        row: impl Fn(usize) -> U + 'static,
    ) -> DynamicSegment<R> {
        let run = self.clone();
        DynamicSegment::new(move |slot: ChildSlot<R>| {
            let held: HeldEach<T, R> = store(RefCell::new(Vec::new()));
            create_binding(move || {
                let held = held.value();
                let Some(states) = states.try_value() else {
                    return;
                };
                let items = run.items();
                let mut previous = std::mem::take(&mut *held.borrow_mut());
                let mut gone = previous.split_off(previous.len().min(items.len()));
                states.borrow_mut().truncate(items.len());
                let mut kept = previous.into_iter();
                let mut next = Vec::with_capacity(items.len());
                for (index, item) in items.into_iter().enumerate() {
                    match kept.next() {
                        Some(row) if Rc::ptr_eq(&row.0, &item) => next.push(row),
                        replaced => {
                            gone.extend(replaced);
                            let scope = Scope::detached();
                            let stored = scope.context().run(|| {
                                let made = state(index, item.clone());
                                let mut states = states.borrow_mut();
                                match states.get_mut(index) {
                                    Some(held) => *held = made,
                                    None => states.push(made),
                                }
                                drop(states);
                                slot.store(row(index).into_child())
                            });
                            next.push((item, stored, scope));
                        }
                    }
                }
                slot.fill(next.iter().map(|(_, stored, _)| stored.clone()).collect());
                *held.borrow_mut() = next;
                drop(gone);
            });
        })
    }
}

type HeldKeyed<T, R> =
    Stored<RefCell<Vec<(Rc<T>, <R as SlotChild>::Stored, Scope, Signal<usize>)>>>;

impl<T: 'static, C: SlotChild<Stored = Rc<T>>> Run<C> {
    pub fn build_keyed<R: SlotChild, U: IntoChild<R>>(
        &self,
        row: impl Fn(Rc<T>, ReadSignal<usize>) -> U + 'static,
    ) -> DynamicSegment<R> {
        let run = self.clone();
        DynamicSegment::new(move |slot: ChildSlot<R>| {
            let held: HeldKeyed<T, R> = store(RefCell::new(Vec::new()));
            create_binding(move || {
                let held = held.value();
                let items = run.items();
                let mut previous: Vec<_> = std::mem::take(&mut *held.borrow_mut())
                    .into_iter()
                    .map(Some)
                    .collect();
                let mut next = Vec::with_capacity(items.len());
                for (index, item) in items.into_iter().enumerate() {
                    let kept = previous
                        .iter_mut()
                        .find_map(|row| row.take_if(|(built, ..)| Rc::ptr_eq(built, &item)));
                    match kept {
                        Some(row) => {
                            row.3.set(index);
                            next.push(row);
                        }
                        None => {
                            let scope = Scope::detached();
                            let (stored, at) = scope.context().run(|| {
                                let at = create_signal(index);
                                let built = row(item.clone(), at.read_only()).into_child();
                                (slot.store(built), at)
                            });
                            next.push((item, stored, scope, at));
                        }
                    }
                }
                slot.fill(next.iter().map(|(_, stored, ..)| stored.clone()).collect());
                *held.borrow_mut() = next;
                drop(previous);
            });
        })
    }
}

impl<C: SlotChild> IntoProp<Vec<C::Stored>> for Run<C> {
    fn into_prop(self) -> Prop<Vec<C::Stored>> {
        Prop::dynamic(move || self.items())
    }
}

enum Place<C: SlotChild> {
    Host {
        host: Rc<dyn SlotHost<C>>,
        slot: SlotId,
    },
    Run {
        run: Rc<RunState<C::Stored>>,
        slot: SlotId,
    },
}

impl<C: SlotChild> Clone for Place<C> {
    fn clone(&self) -> Self {
        match self {
            Place::Host { host, slot } => Place::Host {
                host: host.clone(),
                slot: *slot,
            },
            Place::Run { run, slot } => Place::Run {
                run: run.clone(),
                slot: *slot,
            },
        }
    }
}

pub struct ChildSlot<C: SlotChild> {
    place: Place<C>,
    change: Option<Rc<dyn Fn(C) -> C>>,
}

impl<C: SlotChild> Clone for ChildSlot<C> {
    fn clone(&self) -> Self {
        Self {
            place: self.place.clone(),
            change: self.change.clone(),
        }
    }
}

impl<C: SlotChild> ChildSlot<C> {
    /// The slot this fills in the host it was opened in, if it was opened in a host.
    pub(crate) fn host_slot(&self) -> Option<SlotId> {
        match &self.place {
            Place::Host { slot, .. } => Some(*slot),
            Place::Run { .. } => None,
        }
    }

    pub(crate) fn in_host(host: Rc<dyn SlotHost<C>>) -> Self {
        let slot = host.open();
        Self {
            place: Place::Host { host, slot },
            change: None,
        }
    }

    fn in_run(run: &Run<C>) -> Self {
        let slot = run.state.items.borrow_mut().open();
        Self {
            place: Place::Run {
                run: run.state.clone(),
                slot,
            },
            change: None,
        }
    }

    fn mapping(self, change: Rc<dyn Fn(C) -> C>) -> Self {
        let inner = self.change;
        let change: Rc<dyn Fn(C) -> C> = match inner {
            Some(inner) => Rc::new(move |child| change(inner(child))),
            None => change,
        };
        Self {
            place: self.place,
            change: Some(change),
        }
    }

    pub fn store(&self, child: C) -> C::Stored {
        let child = match &self.change {
            Some(change) => change(child),
            None => child,
        };
        match &self.place {
            Place::Host { host, .. } => host.store(child),
            Place::Run { .. } => child.store(),
        }
    }

    pub fn fill(&self, items: Vec<C::Stored>) {
        match &self.place {
            Place::Host { host, slot } => host.fill(*slot, items),
            Place::Run { run, slot } => {
                run.items.borrow_mut().fill(*slot, items);
                run.changed();
            }
        }
    }
}

pub(crate) struct NestedHost<C: SlotChild> {
    slot: ChildSlot<C>,
    items: RefCell<ChildList<C::Stored>>,
    mounting: Cell<bool>,
    visible: Cell<bool>,
}

impl<C: SlotChild> NestedHost<C> {
    pub(crate) fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        if visible {
            self.publish();
        } else {
            self.slot.fill(Vec::new());
        }
    }

    fn publish(&self) {
        if !self.mounting.get() && self.visible.get() {
            self.slot
                .fill(self.items.borrow().iter().cloned().collect());
        }
    }
}

impl<C: SlotChild> SlotHost<C> for NestedHost<C> {
    fn store(&self, child: C) -> C::Stored {
        self.slot.store(child)
    }
    fn append(&self, stored: C::Stored) {
        self.items.borrow_mut().push(stored);
        self.publish();
    }
    fn open(&self) -> SlotId {
        self.items.borrow_mut().open()
    }
    fn fill(&self, slot: SlotId, items: Vec<C::Stored>) {
        self.items.borrow_mut().fill(slot, items);
        self.publish();
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be written between these tags",
    label = "this component takes `{T}` children"
)]
pub trait IntoSegment<T: SlotChild> {
    fn into_segment(self) -> ChildSegment<T>;
}

pub fn into_segment<T: SlotChild>(child: impl IntoSegment<T>) -> ChildSegment<T> {
    child.into_segment()
}

impl<T: SlotChild> IntoSegment<T> for Children<T> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Nested(self.0)
    }
}

impl<T: SlotChild> IntoSegment<T> for Option<Children<T>> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Nested(self.map(|children| children.0).unwrap_or_default())
    }
}

#[macro_export]
macro_rules! child_type {
    (<$($generic:ident $(: $bound:path)?),*> $ty:ty) => {
        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::IntoChild<$ty> for $ty {
            fn into_child(self) -> $ty {
                self
            }
        }

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::IntoSegment<$ty> for $ty {
            fn into_segment(self) -> $crate::reactive::ChildSegment<$ty> {
                $crate::reactive::ChildSegment::One(self)
            }
        }

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::IntoSegment<$ty> for ::core::option::Option<$ty> {
            fn into_segment(self) -> $crate::reactive::ChildSegment<$ty> {
                $crate::reactive::ChildSegment::Many(self.into_iter().collect())
            }
        }

        $crate::one_child_slot!(<$($generic $(: $bound)?),*> $ty);

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::Counted for $ty {
            type Count = $crate::reactive::CountOne;
        }

        impl<$($generic: 'static $(+ $bound)?),*> ::core::convert::From<$ty> for $crate::reactive::Children<$ty> {
            fn from(child: $ty) -> Self {
                Self::from($crate::reactive::ChildSegment::One(child))
            }
        }

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::Takes<$ty, ()> for $crate::reactive::Children<$ty> {
            fn take(child: $ty) -> Self {
                Self::from($crate::reactive::ChildSegment::One(child))
            }
        }
    };
    ($ty:ty) => {
        $crate::child_type!(<> $ty);
    };
}

#[macro_export]
macro_rules! value_child_type {
    (<$($generic:ident $(: $bound:path)?),*> $ty:ty) => {
        $crate::child_type!(<$($generic $(: $bound)?),*> $ty);

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::ChildValue for $ty {}

        impl<$($generic: 'static $(+ $bound)?),*> $crate::reactive::SlotChild for $ty {
            type Stored = ::std::rc::Rc<$ty>;
            type Node = ::std::rc::Rc<$ty>;

            fn store(self) -> Self::Stored {
                ::std::rc::Rc::new(self)
            }

            fn node(stored: &Self::Stored) -> Self::Node {
                stored.clone()
            }
        }
    };
    ($ty:ty) => {
        $crate::value_child_type!(<> $ty);
    };
}

pub struct Children<T: SlotChild>(Vec<ChildSegment<T>>);

#[diagnostic::on_unimplemented(
    message = "a `{Self}` does not name a kind of child",
    label = "type this prop `Children<_>`"
)]
pub trait ChildrenSlot {
    type Child: SlotChild;
}

impl<T: SlotChild> ChildValue for Children<T> {}

impl<T: SlotChild, H: IntoSegment<T>> FromIterator<H> for Children<T> {
    fn from_iter<I: IntoIterator<Item = H>>(iter: I) -> Self {
        Self(iter.into_iter().map(IntoSegment::into_segment).collect())
    }
}

impl<T: SlotChild> ChildrenSlot for Children<T> {
    type Child = T;
}

impl<T: SlotChild> Default for Children<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: SlotChild, const N: usize> From<[ChildSegment<T>; N]> for Children<T> {
    fn from(segments: [ChildSegment<T>; N]) -> Self {
        Self(Vec::from(segments))
    }
}

impl<T: SlotChild> From<ChildSegment<T>> for Children<T> {
    fn from(segment: ChildSegment<T>) -> Self {
        Self(vec![segment])
    }
}

impl<T: SlotChild> From<Run<T>> for Children<T> {
    fn from(run: Run<T>) -> Self {
        Self::from(ChildSegment::Dynamic(DynamicSegment::new(
            move |slot: ChildSlot<T>| {
                let held = slot.clone();
                held.fill(run.get_untracked());
                create_binding(move || slot.fill(run.items()));
            },
        )))
    }
}

/// Anything that fills a run fills a `Children` built at once (a render prop's).
impl<T: SlotChild, H: IntoSegment<T>> IntoChild<Children<T>> for H {
    fn into_child(self) -> Children<T> {
        Children(vec![self.into_segment()])
    }
}

impl<T: SlotChild> From<DynamicSegment<T>> for Children<T> {
    fn from(segment: DynamicSegment<T>) -> Self {
        Self::from(segment.into_segment())
    }
}

impl<T: SlotChild> Children<T> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn map(self, change: impl Fn(T) -> T + 'static) -> Self {
        let change: Rc<dyn Fn(T) -> T> = Rc::new(change);
        Self(
            self.0
                .into_iter()
                .map(|segment| segment.map(&change))
                .collect(),
        )
    }

    pub fn into_only(mut self) -> Result<T, Self> {
        match self.0.as_slice() {
            [ChildSegment::One(_)] => match self.0.pop() {
                Some(ChildSegment::One(child)) => Ok(child),
                _ => unreachable!(),
            },
            [ChildSegment::Nested(_)] => match self.0.pop() {
                Some(ChildSegment::Nested(segments)) => Self(segments).into_only(),
                _ => unreachable!(),
            },
            _ => Err(self),
        }
    }

    /// How a component reads a slot of value children (`MenuItem`s, say).
    pub fn into_run(self) -> Run<T> {
        let run = Run::default();
        for segment in self.0 {
            fill_run(&run, segment);
        }
        run
    }

    pub fn mount(self, host: impl IntoSlotHost<T>) {
        let host = host.into_slot_host();
        for segment in self.0 {
            mount_segment(&host, segment);
        }
    }

    pub(crate) fn mount_in(self, slot: ChildSlot<T>) -> Rc<NestedHost<T>> {
        self.mount_nested(slot, true)
    }

    pub(crate) fn mount_hidden(self, slot: ChildSlot<T>) -> Rc<NestedHost<T>> {
        self.mount_nested(slot, false)
    }

    fn mount_nested(self, slot: ChildSlot<T>, visible: bool) -> Rc<NestedHost<T>> {
        let host = Rc::new(NestedHost {
            slot,
            items: RefCell::default(),
            mounting: Cell::new(true),
            visible: Cell::new(visible),
        });
        self.mount(host.clone() as Rc<dyn SlotHost<T>>);
        host.mounting.set(false);
        host.publish();
        host
    }

    pub fn from_block(block: impl IntoSegments<T>) -> Self {
        let mut segments = Vec::new();
        block.into_segments(&mut segments);
        Self(segments)
    }
}

fn fill_run<T: SlotChild>(run: &Run<T>, segment: ChildSegment<T>) {
    match segment {
        ChildSegment::One(child) => run.state.items.borrow_mut().push(child.store()),
        ChildSegment::Many(children) => {
            for child in children {
                run.state.items.borrow_mut().push(child.store());
            }
        }
        ChildSegment::Nested(segments) => {
            for segment in segments {
                fill_run(run, segment);
            }
        }
        ChildSegment::Dynamic(segment) => segment.mount(ChildSlot::in_run(run)),
    }
}

fn mount_segment<T: SlotChild>(host: &Rc<dyn SlotHost<T>>, segment: ChildSegment<T>) {
    match segment {
        ChildSegment::One(child) => host.append(host.store(child)),
        ChildSegment::Many(children) => {
            for child in children {
                host.append(host.store(child));
            }
        }
        ChildSegment::Nested(segments) => {
            for segment in segments {
                mount_segment(host, segment);
            }
        }
        ChildSegment::Dynamic(segment) => segment.mount(ChildSlot::in_host(host.clone())),
    }
}

#[diagnostic::on_unimplemented(
    message = "these cannot be written between these tags",
    label = "this component takes `{T}` children"
)]
pub trait IntoSegments<T: SlotChild> {
    fn into_segments(self, segments: &mut Vec<ChildSegment<T>>);
}

pub struct SingleChild<H>(pub H);

/// What a component that wraps its children without placing them (to provide context around
/// them, say) returns: its one child as it is, so a node stays a node, or its run of children.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be handed back as it was built",
    label = "write the children as tags, or as a block that returns a child"
)]
pub trait Unwrapped {
    type Output: ChildValue;

    fn unwrapped(self) -> Self::Output;
}

impl<H: ChildValue> Unwrapped for SingleChild<H> {
    type Output = H;

    fn unwrapped(self) -> H {
        self.0
    }
}

impl<T: SlotChild, const N: usize> Unwrapped for [ChildSegment<T>; N] {
    type Output = Children<T>;

    fn unwrapped(self) -> Children<T> {
        let mut segments = Vec::with_capacity(N);
        self.into_segments(&mut segments);
        Children(segments)
    }
}

impl<H: ChildValue> Unwrapped for H {
    type Output = H;

    fn unwrapped(self) -> H {
        self
    }
}

impl<T: SlotChild, H: IntoSegment<T>> IntoSegment<T> for SingleChild<H> {
    fn into_segment(self) -> ChildSegment<T> {
        self.0.into_segment()
    }
}

impl<T: SlotChild, const N: usize> IntoSegment<T> for [ChildSegment<T>; N] {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Nested(Vec::from(self))
    }
}

impl<T: SlotChild, H: IntoSegment<T>> IntoSegments<T> for SingleChild<H> {
    fn into_segments(self, segments: &mut Vec<ChildSegment<T>>) {
        segments.push(self.0.into_segment());
    }
}

impl<T: SlotChild, const N: usize> IntoSegments<T> for [ChildSegment<T>; N] {
    fn into_segments(self, segments: &mut Vec<ChildSegment<T>>) {
        segments.extend(self);
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds exactly one child",
    label = "write one child between these tags"
)]
pub trait OneChild<T> {
    fn one_child(self) -> T;
}

impl<T, H: IntoChild<T>> OneChild<T> for SingleChild<H> {
    fn one_child(self) -> T {
        self.0.into_child()
    }
}

/// Several roots, where a render builds a `Children` at once.
impl<T: SlotChild, const N: usize> OneChild<Children<T>> for [ChildSegment<T>; N] {
    fn one_child(self) -> Children<T> {
        Children(Vec::from(self))
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds at most one child",
    label = "write one child between these tags, or none at all"
)]
pub trait AtMostOneChild<T> {
    fn at_most_one_child(self) -> Option<T>;
}

impl<T: SlotChild> AtMostOneChild<T> for [ChildSegment<T>; 0] {
    fn at_most_one_child(self) -> Option<T> {
        None
    }
}

impl<T, H: IntoChild<T>> AtMostOneChild<T> for SingleChild<H> {
    fn at_most_one_child(self) -> Option<T> {
        Some(self.0.into_child())
    }
}
