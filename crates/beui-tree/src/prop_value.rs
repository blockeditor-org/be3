//! What a `#[component]` prop takes. Every setter the macro writes asks for
//! `IntoPropValue<P, M>`, where `P` is the prop's declared type: the macro does not look at
//! that type, so each prop type says here what it takes. `M` is a marker that keeps the
//! impls for one `P` apart; nothing names it.
//!
//! Every type takes itself (`Itself`). The other impls of a type are kept from overlapping
//! that one: a `Prop<T>` takes what `IntoPropFrom<T, M>` takes with a marker that is a
//! `ValueMarker`, and an `Option<X>` takes what `X` takes the same way.
//!
//! Children written between a tag's tags reach the same setter wrapped by `view!`, by the
//! shape they are written in: `ChildBlock` (tags), `ChildClosure` (`|value| { .. }`),
//! `ChildUnitClosure` (`|| { .. }`) and `ChildWith` (`|value| <Tag />`). A slot type takes the
//! shapes it implements `TakesBlock`, `TakesClosure`, `TakesUnitClosure` or `TakesWith` for.

use beui_reactive::{Memo, ReadSignal, Reader, Signal, Trackable, Trigger, create_signal};

use crate::binding::Binding;
use crate::callback::Callback;
use crate::children::{
    ChildSegment, Children, DynamicSegment, IntoChild, IntoSegments, OneChild,
    Run, SlotChild,
};
use crate::component::{Render, RenderFn};
use crate::prop::{
    ByClosure, ByValue, Func, Given, IntoPropFrom, IntoReadProp, Prop,
};

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be passed as a `{P}` prop",
    label = "this is a `{Self}`"
)]
pub trait IntoPropValue<P, M> {
    fn into_prop_value(self) -> P;
}

/// A prop whose child or value type is one of its component's own type parameters: it takes
/// what it is passed without converting it, so that parameter follows what is passed.
pub struct Exact<P>(pub P);

/// The value of a prop left out of a tag, for the types that have one. A prop of any other
/// type, without a `#[prop(default = ..)]`, must be given.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` prop has no value when it is left out",
    label = "this tag leaves out a prop it must set",
    note = "set the prop in the tag, or give it a `#[prop(default = ...)]`"
)]
pub trait AbsentProp: Sized {
    fn absent() -> Self;
}

/// A prop a tag has set, as the builder keeps it until `build`.
pub struct SetProp<T>(pub T);

pub struct Itself;
pub struct ByUnitClosure;
pub struct BySignal;
pub struct ByInto;
pub struct ByProp<M>(std::marker::PhantomData<M>);
pub struct ByExact<M>(std::marker::PhantomData<M>);
pub struct ByBlock;
pub struct ByChildClosure;
pub struct ByChildUnitClosure;
pub struct ByChildWith;

/// The marker of a value's conversion, as opposed to a child shape `view!` wraps: an
/// `Option<X>` takes what `X` takes by value, and none of the shapes, which it takes itself.
pub struct Val<M>(std::marker::PhantomData<M>);

/// The marker of a child shape `view!` wraps. The children setter asks for a `ChildShape<..>`
/// marker, so only the slot type's own `Takes..` impls answer for the children, and their
/// diagnostics are the ones shown.
pub struct ChildShape<M>(std::marker::PhantomData<M>);

#[diagnostic::do_not_recommend]
impl<P> IntoPropValue<P, Val<Itself>> for P {
    fn into_prop_value(self) -> P {
        self
    }
}

// Prop<T>

/// What `IntoPropFrom` takes: everything but a `Prop<T>` itself, which every type takes.
impl<T, V: IntoPropFrom<T, M>, M> IntoPropValue<Prop<T>, Val<ByProp<M>>> for V {
    fn into_prop_value(self) -> Prop<T> {
        self.into_prop_from()
    }
}

/// Any type passed as itself where its exact type is taken.
#[diagnostic::do_not_recommend]
impl<P> IntoPropValue<Exact<P>, Val<Itself>> for P {
    fn into_prop_value(self) -> Exact<P> {
        Exact(self)
    }
}

impl<T, M, V: IntoReadProp<T, M>> IntoPropValue<Exact<Prop<T>>, Val<ByExact<M>>> for V {
    fn into_prop_value(self) -> Exact<Prop<T>> {
        Exact(self.into_read_prop())
    }
}

// Option<X>

#[diagnostic::do_not_recommend]
impl<X, V, M> IntoPropValue<Option<X>, Val<Given<M>>> for V
where
    V: IntoPropValue<X, Val<M>>,
{
    fn into_prop_value(self) -> Option<X> {
        Some(self.into_prop_value())
    }
}

impl<T> AbsentProp for Option<T> {
    fn absent() -> Self {
        None
    }
}

// String

macro_rules! into_string {
    ($($ty:ty),*) => {$(
        impl IntoPropValue<String, Val<ByInto>> for $ty {
            fn into_prop_value(self) -> String {
                self.into()
            }
        }
    )*};
}

into_string!(&str, &String, &mut str, char, Box<str>, std::borrow::Cow<'_, str>);

// Callback<V, R>

impl<V: 'static, R: 'static, F: FnMut(V) -> R + 'static> IntoPropValue<Callback<V, R>, Val<ByClosure>>
    for F
{
    fn into_prop_value(self) -> Callback<V, R> {
        Callback::new(self)
    }
}

impl<F: FnMut() + 'static> IntoPropValue<Callback<()>, Val<ByUnitClosure>> for F {
    fn into_prop_value(mut self) -> Callback<()> {
        Callback::new(move |()| self())
    }
}

impl<V: PartialEq + 'static> IntoPropValue<Callback<V>, Val<BySignal>> for Signal<V> {
    fn into_prop_value(self) -> Callback<V> {
        Callback::from(self)
    }
}

impl<T: Clone + PartialEq + 'static> IntoPropValue<Callback<T>, Val<BySignal>> for Binding<T> {
    fn into_prop_value(self) -> Callback<T> {
        crate::callback::IntoCallback::into_callback(self)
    }
}

impl<V, R> AbsentProp for Callback<V, R> {
    fn absent() -> Self {
        Callback::default()
    }
}

// Func<V, R>

impl<V: 'static, R: 'static, F: Fn(V) -> R + 'static> IntoPropValue<Func<V, R>, Val<ByClosure>> for F {
    fn into_prop_value(self) -> Func<V, R> {
        Func::new(self)
    }
}

// Binding<T>

impl<T: 'static> IntoPropValue<Binding<T>, Val<BySignal>> for Signal<T> {
    fn into_prop_value(self) -> Binding<T> {
        Binding::from(self)
    }
}

impl<T: 'static> IntoPropValue<Binding<T>, Val<ByValue>> for T {
    fn into_prop_value(self) -> Binding<T> {
        Binding::from(create_signal(self))
    }
}

impl IntoPropValue<Binding<String>, Val<ByInto>> for &'static str {
    fn into_prop_value(self) -> Binding<String> {
        Binding::from(create_signal(self.to_string()))
    }
}

/// A binding left out is a signal of its type's default, owned by the component: the macro
/// asks for it inside the component's scope.
impl<T: Default + 'static> AbsentProp for Binding<T> {
    fn absent() -> Self {
        Binding::from(create_signal(T::default()))
    }
}

// Render<H, C> and RenderFn<H, C>: a closure taking the handle, or one ignoring it, whose
// value converts into the child.

impl<H: 'static, C: 'static, R: IntoChild<C>, F: Fn(H) -> R + 'static>
    IntoPropValue<RenderFn<H, C>, Val<ByClosure>> for F
{
    fn into_prop_value(self) -> RenderFn<H, C> {
        RenderFn::new(move |handle| self(handle).into_child())
    }
}

impl<H: 'static, C: 'static, R: IntoChild<C>, F: Fn() -> R + 'static>
    IntoPropValue<RenderFn<H, C>, Val<ByUnitClosure>> for F
{
    fn into_prop_value(self) -> RenderFn<H, C> {
        RenderFn::new(move |_| self().into_child())
    }
}

impl<H, C, R: IntoChild<C>, F: FnOnce(H) -> R + 'static> IntoPropValue<Render<H, C>, Val<ByClosure>>
    for F
{
    fn into_prop_value(self) -> Render<H, C> {
        Render::new(move |handle| self(handle).into_child())
    }
}

impl<H, C, R: IntoChild<C>, F: FnOnce() -> R + 'static> IntoPropValue<Render<H, C>, Val<ByUnitClosure>>
    for F
{
    fn into_prop_value(self) -> Render<H, C> {
        Render::new(move |_| self().into_child())
    }
}

// The same, raw: the closure's value is the child as it is.

impl<H: 'static, C: 'static, F: Fn(H) -> C + 'static> IntoPropValue<Exact<RenderFn<H, C>>, Val<ByClosure>>
    for F
{
    fn into_prop_value(self) -> Exact<RenderFn<H, C>> {
        Exact(RenderFn::new(self))
    }
}

impl<H: 'static, C: 'static, F: Fn() -> C + 'static>
    IntoPropValue<Exact<RenderFn<H, C>>, Val<ByUnitClosure>> for F
{
    fn into_prop_value(self) -> Exact<RenderFn<H, C>> {
        Exact(RenderFn::new(move |_| self()))
    }
}


impl<H, C, F: FnOnce(H) -> C + 'static> IntoPropValue<Exact<Render<H, C>>, Val<ByClosure>> for F {
    fn into_prop_value(self) -> Exact<Render<H, C>> {
        Exact(Render::new(self))
    }
}

impl<H, C, F: FnOnce() -> C + 'static> IntoPropValue<Exact<Render<H, C>>, Val<ByUnitClosure>> for F {
    fn into_prop_value(self) -> Exact<Render<H, C>> {
        Exact(Render::new(move |_| self()))
    }
}


// Children<C>

impl<C: SlotChild> IntoPropValue<Children<C>, Val<ByInto>> for Run<C> {
    fn into_prop_value(self) -> Children<C> {
        self.into()
    }
}

impl<C: SlotChild> IntoPropValue<Children<C>, Val<ByInto>> for DynamicSegment<C> {
    fn into_prop_value(self) -> Children<C> {
        self.into()
    }
}

impl<C: SlotChild> IntoPropValue<Children<C>, Val<ByInto>> for ChildSegment<C> {
    fn into_prop_value(self) -> Children<C> {
        self.into()
    }
}

impl<C: SlotChild> AbsentProp for Children<C> {
    fn absent() -> Self {
        Children::default()
    }
}

// Trackable: what it converts from.

macro_rules! into_trackable {
    ($($ty:ty),*) => {$(
        impl<T: 'static> IntoPropValue<Trackable, Val<ByInto>> for $ty {
            fn into_prop_value(self) -> Trackable {
                self.into()
            }
        }
    )*};
}

into_trackable!(Signal<T>, ReadSignal<T>, Memo<T>, Reader<T>, Binding<T>);

impl IntoPropValue<Trackable, Val<ByInto>> for Trigger {
    fn into_prop_value(self) -> Trackable {
        self.into()
    }
}

// The child shapes `view!` writes.

/// Children written as tags: `move || <what they build>`.
pub struct ChildBlock<F>(pub F);
/// `|value| { .. }`
pub struct ChildClosure<F>(pub F);
/// `|| { .. }`
pub struct ChildUnitClosure<F>(pub F);
/// `|value| <Tag />`: `move |value| <what the tags build>`.
pub struct ChildWith<F>(pub F);

/// The children slot of type `P`, as `view!` hands it to the closure it passes `__children`.
/// A closure child is made through a method on it, `closure` or `with`, and which trait's
/// method that is follows from `P`: `FnSlot` for a slot that calls its render again
/// (`RenderFn`), `FnOnceSlot` for one that calls it once (`Render`). Its bound is an `Fn`
/// trait of the handle the slot hands, so the closure's parameters are known while its body
/// is checked, as they would be against a setter written for that slot. A slot that takes no
/// closure gets `NoHandedSlot`'s, which leaves the error to `TakesClosure` or `TakesWith`.
pub struct Slot<P>(std::marker::PhantomData<fn() -> P>);

impl<P> Slot<P> {
    pub fn new() -> Self {
        Slot(std::marker::PhantomData)
    }
}

impl<P> Default for Slot<P> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait FnSlot {
    type Handed;

    fn closure<F: Fn(Self::Handed) -> R, R>(self, closure: F) -> ChildClosure<F>
    where
        Self: Sized,
    {
        ChildClosure(closure)
    }

    fn with<F: Fn(Self::Handed) -> R, R>(self, closure: F) -> ChildWith<F>
    where
        Self: Sized,
    {
        ChildWith(closure)
    }
}

pub trait FnOnceSlot {
    type Handed;

    fn closure<F: FnOnce(Self::Handed) -> R, R>(self, closure: F) -> ChildClosure<F>
    where
        Self: Sized,
    {
        ChildClosure(closure)
    }

    fn with<F: FnOnce(Self::Handed) -> R, R>(self, closure: F) -> ChildWith<F>
    where
        Self: Sized,
    {
        ChildWith(closure)
    }
}

/// Any other slot, reached by autoref only when neither trait above has it.
pub trait NoHandedSlot {
    fn closure<F>(self, closure: F) -> ChildClosure<F>
    where
        Self: Sized,
    {
        ChildClosure(closure)
    }

    fn with<F>(self, closure: F) -> ChildWith<F>
    where
        Self: Sized,
    {
        ChildWith(closure)
    }
}

impl<P> NoHandedSlot for &Slot<P> {}

impl<H, C> FnSlot for Slot<RenderFn<H, C>> {
    type Handed = H;
}

impl<H, C> FnSlot for Slot<Exact<RenderFn<H, C>>> {
    type Handed = H;
}

impl<H, C> FnSlot for Slot<Option<RenderFn<H, C>>> {
    type Handed = H;
}

impl<H, C> FnOnceSlot for Slot<Render<H, C>> {
    type Handed = H;
}

impl<H, C> FnOnceSlot for Slot<Exact<Render<H, C>>> {
    type Handed = H;
}

impl<H, C> FnOnceSlot for Slot<Option<Render<H, C>>> {
    type Handed = H;
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` slot cannot hold the children written between these tags",
    label = "these children"
)]
pub trait TakesBlock<F> {
    fn take_block(block: F) -> Self;
}

#[diagnostic::on_unimplemented(
    message = "this component takes its children already built, so they cannot be a block that builds them",
    label = "write these children as tags or as `{{expr}}`, without `|value| {{...}}`"
)]
pub trait TakesClosure<F> {
    fn take_closure(closure: F) -> Self;
}

#[diagnostic::on_unimplemented(
    message = "this component takes its children already built, so they cannot be a block that builds them",
    label = "write these children as tags or as `{{expr}}`, without `|| {{...}}`"
)]
pub trait TakesUnitClosure<F> {
    fn take_unit_closure(closure: F) -> Self;
}

#[diagnostic::on_unimplemented(
    message = "this component hands nothing to its children, so there is no value to name",
    label = "write these children without `|value|`"
)]
pub trait TakesWith<F> {
    fn take_with(closure: F) -> Self;
}

impl<P: TakesBlock<F>, F> IntoPropValue<P, ChildShape<ByBlock>> for ChildBlock<F> {
    fn into_prop_value(self) -> P {
        P::take_block(self.0)
    }
}

impl<P: TakesClosure<F>, F> IntoPropValue<P, ChildShape<ByChildClosure>> for ChildClosure<F> {
    fn into_prop_value(self) -> P {
        P::take_closure(self.0)
    }
}

impl<P: TakesUnitClosure<F>, F> IntoPropValue<P, ChildShape<ByChildUnitClosure>> for ChildUnitClosure<F> {
    fn into_prop_value(self) -> P {
        P::take_unit_closure(self.0)
    }
}

impl<P: TakesWith<F>, F> IntoPropValue<P, ChildShape<ByChildWith>> for ChildWith<F> {
    fn into_prop_value(self) -> P {
        P::take_with(self.0)
    }
}

impl<C: SlotChild, F: FnOnce() -> B, B: IntoSegments<C>> TakesBlock<F> for Children<C> {
    fn take_block(block: F) -> Self {
        Children::from_block(block())
    }
}

/// A slot typed by the one child it holds takes a block that builds one; the child type
/// says so with `one_child_slot!` (which `child_type!` writes), so that each slot type has an
/// impl of its own and a wrong block reaches `OneChild`'s or `AtMostOneChild`'s diagnostic.
#[macro_export]
macro_rules! one_child_slot {
    (<$($generic:ident $(: $bound:path)?),*> $ty:ty) => {
        impl<$($generic: 'static $(+ $bound)?,)* __F: ::core::ops::FnOnce() -> __B, __B: $crate::reactive::OneChild<$ty>>
            $crate::reactive::TakesBlock<__F> for $ty
        {
            fn take_block(block: __F) -> Self {
                $crate::reactive::OneChild::one_child(block())
            }
        }

        impl<$($generic: 'static $(+ $bound)?,)* __F: ::core::ops::FnOnce() -> __B, __B: $crate::reactive::AtMostOneChild<$ty>>
            $crate::reactive::TakesOptionalBlock<__F> for $ty
        {
            fn take_optional_block(block: __F) -> ::core::option::Option<Self> {
                $crate::reactive::AtMostOneChild::at_most_one_child(block())
            }
        }
    };
}

/// An optional slot: at most one child, or an optional render built from the block. One impl
/// for every `Option`, so a wrong block reaches the inner type's diagnostic.
impl<X: TakesOptionalBlock<F>, F> TakesBlock<F> for Option<X> {
    fn take_block(block: F) -> Self {
        X::take_optional_block(block)
    }
}

#[diagnostic::on_unimplemented(
    message = "an optional `{Self}` slot cannot hold the children written between these tags",
    label = "these children"
)]
pub trait TakesOptionalBlock<F>: Sized {
    fn take_optional_block(block: F) -> Option<Self>;
}


macro_rules! render_slot {
    ($render:ident, $fn:ident, ($($static:tt)*)) => {
        impl<H $($static)*, C $($static)*, F: $fn() -> B + 'static, B: OneChild<C>> TakesBlock<F>
            for $render<H, C>
        {
            fn take_block(block: F) -> Self {
                $render::new(move |_| block().one_child())
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn(H) -> B + 'static, B: IntoChild<C>> TakesClosure<F>
            for $render<H, C>
        {
            fn take_closure(closure: F) -> Self {
                $render::new(move |handle| closure(handle).into_child())
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn() -> B + 'static, B: IntoChild<C>>
            TakesUnitClosure<F> for $render<H, C>
        {
            fn take_unit_closure(closure: F) -> Self {
                $render::new(move |_| closure().into_child())
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn(H) -> B + 'static, B: OneChild<C>> TakesWith<F>
            for $render<H, C>
        {
            fn take_with(closure: F) -> Self {
                $render::new(move |handle| closure(handle).one_child())
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn() -> C + 'static> TakesBlock<F>
            for Exact<$render<H, C>>
        {
            fn take_block(block: F) -> Self {
                Exact($render::new(move |_| block()))
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn(H) -> C + 'static> TakesClosure<F>
            for Exact<$render<H, C>>
        {
            fn take_closure(closure: F) -> Self {
                Exact($render::new(closure))
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn() -> C + 'static> TakesUnitClosure<F>
            for Exact<$render<H, C>>
        {
            fn take_unit_closure(closure: F) -> Self {
                Exact($render::new(move |_| closure()))
            }
        }

        impl<H $($static)*, C $($static)*, F: $fn(H) -> C + 'static> TakesWith<F>
            for Exact<$render<H, C>>
        {
            fn take_with(closure: F) -> Self {
                Exact($render::new(closure))
            }
        }
    };
}

render_slot!(RenderFn, Fn, (: 'static));

/// An optional render slot built from what the render takes.
impl<X: TakesClosure<F>, F> TakesClosure<F> for Option<X> {
    fn take_closure(closure: F) -> Self {
        Some(X::take_closure(closure))
    }
}

impl<X: TakesUnitClosure<F>, F> TakesUnitClosure<F> for Option<X> {
    fn take_unit_closure(closure: F) -> Self {
        Some(X::take_unit_closure(closure))
    }
}

impl<X: TakesWith<F>, F> TakesWith<F> for Option<X> {
    fn take_with(closure: F) -> Self {
        Some(X::take_with(closure))
    }
}

impl<H, C, F> TakesOptionalBlock<F> for RenderFn<H, C>
where
    RenderFn<H, C>: TakesBlock<F>,
{
    fn take_optional_block(block: F) -> Option<Self> {
        Some(RenderFn::take_block(block))
    }
}

impl<H, C, F> TakesOptionalBlock<F> for Render<H, C>
where
    Render<H, C>: TakesBlock<F>,
{
    fn take_optional_block(block: F) -> Option<Self> {
        Some(Render::take_block(block))
    }
}
render_slot!(Render, FnOnce, ());

