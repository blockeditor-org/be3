use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use reactive::{ReadSignal, Scope, WriteSignal, create_effect, create_signal, on_cleanup};

use crate::child_list::{ChildList, SlotId};
use crate::component::ChildValue;
use crate::prop::{IntoProp, Prop};

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is not something a parent can keep a run of",
    label = "say how a run of these is kept with `child_type!` or `value_child_type!`"
)]
pub trait SlotChild: Sized + 'static {
    type Stored: Clone + 'static;

    fn store(self) -> Self::Stored;

    fn discard(_stored: &Self::Stored) {}
}

pub trait SlotHost<C: SlotChild> {
    fn store(&self, child: C) -> C::Stored {
        child.store()
    }

    fn append(&self, stored: C::Stored);

    fn open(&self) -> SlotId;

    fn fill(&self, slot: SlotId, items: Vec<C::Stored>);

    fn adopt_scope(&self, scope: Scope);
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

pub struct DynamicSegment<T: SlotChild> {
    scope: Option<Scope>,
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
            Some(scope) => {
                let held = slot.clone();
                scope.context().run(move || install(slot));
                held.adopt_scope(scope);
            }
            None => install(slot),
        }
    }
}

impl<T: SlotChild> ChildValue for DynamicSegment<T> {
    fn adopt_scope(&mut self, scope: Scope) {
        self.scope = Some(scope);
    }
}

#[diagnostic::on_unimplemented(
    message = "a `Show`, `Dynamic`, `Keyed` or `ForEach` builds a run of children, and this slot takes exactly one child",
    label = "put it in a parent that keeps a run of children, or return it from a component typed `-> DynamicSegment<_>` and write that component where a run of children fits"
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

struct RunState<S> {
    items: RefCell<ChildList<S>>,
    held: RefCell<Vec<Scope>>,
    filled: ReadSignal<u64>,
    fill: WriteSignal<u64>,
}

impl<S> RunState<S> {
    fn new() -> Self {
        let (filled, fill) = create_signal(0);
        Self {
            items: RefCell::new(ChildList::default()),
            held: RefCell::new(Vec::new()),
            filled,
            fill,
        }
    }

    fn changed(&self) {
        self.fill.update(|filled| *filled += 1);
    }
}

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

type HeldRows<R> = Rc<RefCell<Option<(Vec<<R as SlotChild>::Stored>, Scope)>>>;

impl<C: SlotChild> Run<C> {
    pub fn items(&self) -> Vec<C::Stored> {
        self.state.filled.get();
        self.peek()
    }

    pub fn peek(&self) -> Vec<C::Stored> {
        self.state.items.borrow().iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.state.filled.get();
        self.state.items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn build<R: SlotChild, U: IntoChild<R>>(
        &self,
        rows: impl Fn(Vec<C::Stored>) -> Vec<U> + 'static,
    ) -> DynamicSegment<R> {
        let run = self.clone();
        DynamicSegment::new(move |slot: ChildSlot<R>| {
            let held: HeldRows<R> = Rc::new(RefCell::new(None));
            let owned = held.clone();
            on_cleanup(move || drop(owned));
            create_effect(move || {
                let items = run.items();
                let scope = Scope::detached();
                let built: Vec<R::Stored> = scope.context().run(|| {
                    rows(items)
                        .into_iter()
                        .map(|row| slot.store(row.into_child()))
                        .collect()
                });
                slot.fill(built.clone());
                let previous = held.borrow_mut().replace((built, scope));
                if let Some((stored, scope)) = previous {
                    for row in &stored {
                        R::discard(row);
                    }
                    drop(scope);
                }
            });
        })
    }
}

impl<C: SlotChild> IntoProp<Vec<C::Stored>> for Run<C> {
    fn into_prop(self) -> Prop<Vec<C::Stored>> {
        Prop::Dynamic(Rc::new(move || self.items()))
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
    fn in_host(host: Rc<dyn SlotHost<C>>) -> Self {
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

    pub fn discard(stored: &C::Stored) {
        C::discard(stored);
    }

    fn adopt_scope(&self, scope: Scope) {
        match &self.place {
            Place::Host { host, .. } => host.adopt_scope(scope),
            Place::Run { run, .. } => run.held.borrow_mut().push(scope),
        }
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

#[macro_export]
macro_rules! child_type {
    ($ty:ty) => {
        impl $crate::reactive::IntoChild<$ty> for $ty {
            fn into_child(self) -> $ty {
                self
            }
        }

        impl $crate::reactive::IntoSegment<$ty> for $ty {
            fn into_segment(self) -> $crate::reactive::ChildSegment<$ty> {
                $crate::reactive::ChildSegment::One(self)
            }
        }

        impl ::core::convert::From<$ty> for $crate::reactive::Children<$ty> {
            fn from(child: $ty) -> Self {
                Self::from($crate::reactive::ChildSegment::One(child))
            }
        }
    };
}

#[macro_export]
macro_rules! value_child_type {
    ($ty:ty) => {
        $crate::child_type!($ty);

        impl $crate::reactive::SlotChild for $ty {
            type Stored = ::std::rc::Rc<$ty>;

            fn store(self) -> Self::Stored {
                ::std::rc::Rc::new(self)
            }
        }
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
                held.fill(run.peek());
                create_effect(move || slot.fill(run.items()));
            },
        )))
    }
}

impl<T: SlotChild> From<DynamicSegment<T>> for Children<T> {
    fn from(segment: DynamicSegment<T>) -> Self {
        Self::from(segment.into_segment())
    }
}

impl<T: SlotChild> Children<T> {
    pub fn map(self, change: impl Fn(T) -> T + 'static) -> Self {
        let change: Rc<dyn Fn(T) -> T> = Rc::new(change);
        Self(
            self.0
                .into_iter()
                .map(|segment| segment.map(&change))
                .collect(),
        )
    }

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
