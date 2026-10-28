use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use reactive::{
    Get, ReadSignal, Scope, ScopeContext, Set, Signal, Stored, create_binding, create_signal, store,
};

use crate::child_list::{ChildList, SlotId};
use crate::component::{ChildValue, ComponentContext};
use crate::prop::{IntoProp, Prop};

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is not something a parent can keep a run of",
    label = "say how a run of these is kept with `child_type!` or `value_child_type!`"
)]
pub trait SlotChild: Sized + 'static {
    type Stored: Clone + 'static;

    fn store(self) -> Self::Stored;
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
        self.scope = Some(scope.clone());
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
    filled: Signal<u64>,
}

impl<S> RunState<S> {
    fn new() -> Self {
        Self {
            items: RefCell::new(ChildList::default()),
            filled: create_signal(0),
        }
    }

    fn changed(&self) {
        self.filled.update(|filled| *filled += 1);
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

impl<C: SlotChild> Run<C> {
    #[track_caller]
    pub fn items(&self) -> Vec<C::Stored> {
        self.state.filled.get();
        self.get_untracked()
    }

    pub fn get_untracked(&self) -> Vec<C::Stored> {
        self.state.items.borrow().iter().cloned().collect()
    }

    #[track_caller]
    pub fn len(&self) -> usize {
        self.state.filled.get();
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
                            if row.3.get_untracked() != index {
                                row.3.set(index);
                            }
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

#[macro_export]
macro_rules! child_type {
    (<$($generic:ident),*> $ty:ty) => {
        impl<$($generic: 'static),*> $crate::reactive::IntoChild<$ty> for $ty {
            fn into_child(self) -> $ty {
                self
            }
        }

        impl<$($generic: 'static),*> $crate::reactive::IntoSegment<$ty> for $ty {
            fn into_segment(self) -> $crate::reactive::ChildSegment<$ty> {
                $crate::reactive::ChildSegment::One(self)
            }
        }

        impl<$($generic: 'static),*> ::core::convert::From<$ty> for $crate::reactive::Children<$ty> {
            fn from(child: $ty) -> Self {
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
    (<$($generic:ident),*> $ty:ty) => {
        $crate::child_type!(<$($generic),*> $ty);

        impl<$($generic: 'static),*> $crate::reactive::ChildValue for $ty {}

        impl<$($generic: 'static),*> $crate::reactive::SlotChild for $ty {
            type Stored = ::std::rc::Rc<$ty>;

            fn store(self) -> Self::Stored {
                ::std::rc::Rc::new(self)
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

impl<T: SlotChild> IntoChild<Children<T>> for Children<T> {
    fn into_child(self) -> Children<T> {
        self
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
