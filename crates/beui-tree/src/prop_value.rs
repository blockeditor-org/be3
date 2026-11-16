//! What a `#[component]` prop takes. Every setter the macro writes asks for
//! `IntoPropValue<P, M>`, where `P` is the prop's declared type: the macro does not look at
//! that type, so each prop type says here what it takes. `M` is a marker that keeps the
//! impls for one `P` apart; nothing names it.
//!
//! Every type takes itself (`Itself`). Besides that, each prop type has one impl, keyed on
//! the type, which asks the type what it takes through a trait of its own (`PropTakes` for a
//! `Prop<T>`, `OptionalTakes` for an `Option<X>`, `Takes` for the rest), or asks the value for
//! the one shape it takes (a closure, for a render or `Func`). No impl names the value's type,
//! so a value a prop does not take has one candidate, and rustc reports the obligation that
//! failed under it, with that trait's message, rather than every type `IntoPropValue` has.
//!
//! A prop whose type comes from what is passed asks for `IntoExactProp` instead, which takes
//! only what decides that type (`ExactTakes`).
//!
//! Children written between a tag's tags reach the slot's hidden `__children` setter wrapped
//! by `view!`, by the shape they are written in: `ChildBlock` (tags), `ChildClosure`
//! (`|value| { .. }`), `ChildUnitClosure` (`|| { .. }`) and `ChildWith` (`|value| <Tag />`). It
//! asks the slot type for `TakesChildren`, and so for the shapes it implements `TakesBlock`,
//! `TakesClosure`, `TakesUnitClosure` or `TakesWith` for.

use beui_reactive::{Memo, ReadSignal, Reader, Signal, Trackable, Trigger, create_signal};

use crate::binding::Binding;
use crate::callback::Callback;
use crate::children::{
    ChildSegment, Children, DynamicSegment, IntoChild, IntoSegments, OneChild, Run, SlotChild,
};
use crate::component::{Build, BuildFn, Render, RenderFn};
use crate::prop::{ByClosure, ByValue, Func, Given, IntoPropFrom, IntoReadProp, Prop};

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be passed as a `{P}` prop",
    label = "this is a `{Self}`"
)]
pub trait IntoPropValue<P, M> {
    fn into_prop_value(self) -> P;
}

/// The value of a prop left out of a tag, for the types that have one. A prop of any other
/// type, without a `#[prop(default = ..)]`, must be given.
///
/// Each type that has one also implements `MarkedRequired`, with fewer bounds where its
/// absent value needs some (a `Binding<T>` has one only when `T: Default`), so that
/// `#[prop(required)]` is accepted on it whatever its parameters: the two are written next
/// to each other below.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` prop has no value when it is left out",
    label = "this tag leaves out a prop it must set",
    note = "set the prop in the tag, or give it a `#[prop(default = ...)]`"
)]
pub trait AbsentProp: Sized {
    fn absent() -> Self;
}

/// What `#[prop(required)]` asks of a prop's type: one with an `AbsentProp` value, at least
/// for some of its parameters (a `Binding<T>` has one when `T: Default`), which the tag then
/// sets all the same.
#[diagnostic::on_unimplemented(
    message = "`#[prop(required)]` is for a prop whose type has a value when it is left out (a `Binding<T>`), and a `{Self}` has none",
    label = "this prop is required already: remove `#[prop(required)]`"
)]
pub trait MarkedRequired {}

/// A prop a tag has set, as the builder keeps it until `build`.
pub struct SetProp<T>(pub T);

/// A prop with a `#[prop(default = ..)]`: what was set, or the default.
pub trait PropOr<T> {
    fn prop_or(self, default: impl FnOnce() -> T) -> T;
}

impl<T> PropOr<T> for SetProp<T> {
    fn prop_or(self, _default: impl FnOnce() -> T) -> T {
        self.0
    }
}

impl<T> PropOr<T> for crate::component::MissingProp {
    fn prop_or(self, default: impl FnOnce() -> T) -> T {
        default()
    }
}

/// A type the builder names through a parameter of its own, equal to it, so that its
/// signatures hold no projection of the component's parameters.
pub trait Is<T> {
    fn is(self) -> T;
}

impl<T> Is<T> for T {
    fn is(self) -> T {
        self
    }
}

/// The builder's output parameter, once `build` knows it is the component's output: so the
/// builder type needs none of the component's bounds to name what it builds.
pub trait SameType<T> {
    #[allow(clippy::type_complexity)]
    fn same(binds: Vec<Box<dyn FnOnce(&Self)>>) -> Vec<Box<dyn FnOnce(&T)>>;

    fn from_output(output: T) -> Self;
}

impl<T> SameType<T> for T {
    fn same(binds: Vec<Box<dyn FnOnce(&T)>>) -> Vec<Box<dyn FnOnce(&T)>> {
        binds
    }

    fn from_output(output: T) -> T {
        output
    }
}

pub struct Itself;
pub struct ByUnitClosure;
pub struct BySignal;
pub struct ByInto;
pub struct ByProp<M>(std::marker::PhantomData<M>);
pub struct ByTakes<M>(std::marker::PhantomData<M>);
pub struct ByExact<M>(std::marker::PhantomData<M>);
pub struct BySome;

impl<P> IntoPropValue<P, Itself> for P {
    fn into_prop_value(self) -> P {
        self
    }
}

/// What a prop type takes besides itself, for the types whose impl below asks it: `{Self}` is
/// the prop's type, so a value it does not take is reported against the prop.
#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as a `{Self}` prop",
    label = "this is a `{V}`"
)]
pub trait Takes<V, M>: Sized {
    fn take(value: V) -> Self;
}

// Prop<T>

/// What `IntoPropFrom` takes: everything but a `Prop<T>` itself, which every type takes.
/// It goes through `PropTakes`, a trait of `T`'s, so that a value the prop does not take is
/// reported against `T`.
impl<T, V, M> IntoPropValue<Prop<T>, ByProp<M>> for V
where
    T: PropTakes<V, M>,
{
    fn into_prop_value(self) -> Prop<T> {
        T::take(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as a `{Self}` prop",
    label = "pass a `{Self}`, a signal or memo of one, or a closure returning one"
)]
pub trait PropTakes<V, M>: Sized {
    fn take(value: V) -> Prop<Self>;
}

#[diagnostic::do_not_recommend]
impl<T, V: IntoPropFrom<T, M>, M> PropTakes<V, M> for T {
    fn take(value: V) -> Prop<T> {
        value.into_prop_from()
    }
}

// Option<X>

/// An `Option<X>` takes what an `X` takes, as `Some`, through `OptionalTakes`, a trait of
/// `X`'s, so that a value it does not take is reported as one for an optional `X`.
impl<X, V, M> IntoPropValue<Option<X>, Given<M>> for V
where
    X: OptionalTakes<V, M>,
{
    fn into_prop_value(self) -> Option<X> {
        Some(X::take_some(self))
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as an optional `{Self}` prop",
    label = "pass a `{Self}`, or an `Option<{Self}>` to pass on"
)]
pub trait OptionalTakes<V, M>: Sized {
    fn take_some(value: V) -> Self;
}

impl<X> OptionalTakes<X, Itself> for X {
    fn take_some(value: X) -> X {
        value
    }
}

/// An optional `Prop<T>` goes on to `OptionalPropTakes`, a trait of `T`'s, so that a value it
/// does not take is reported against `T`.
impl<T, V, M> OptionalTakes<V, ByProp<M>> for Prop<T>
where
    T: OptionalPropTakes<V, M>,
{
    fn take_some(value: V) -> Prop<T> {
        T::take(value)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as an optional `{Self}` prop",
    label = "pass a `{Self}`, a signal or memo of one, a closure returning one, or an `Option<Prop<{Self}>>` to pass on"
)]
pub trait OptionalPropTakes<V, M>: Sized {
    fn take(value: V) -> Prop<Self>;
}

#[diagnostic::do_not_recommend]
impl<T, V: IntoPropFrom<T, M>, M> OptionalPropTakes<V, M> for T {
    fn take(value: V) -> Prop<T> {
        value.into_prop_from()
    }
}

/// The types that take values through `Takes`, each by an impl of its own, so that an optional
/// `Prop<T>` has only its own impl to answer for a value.
macro_rules! optional_takes {
    ($(<$($param:ident),*> $ty:ty),*) => {$(
        #[diagnostic::do_not_recommend]
        impl<$($param,)* V, M> OptionalTakes<V, ByTakes<M>> for $ty
        where
            $ty: Takes<V, M>,
        {
            fn take_some(value: V) -> Self {
                <$ty as Takes<V, M>>::take(value)
            }
        }
    )*};
}

optional_takes!(<> String, <A, R> Callback<A, R>, <T> Binding<T>, <> Trackable);

#[diagnostic::do_not_recommend]
impl<C: SlotChild, V, M> OptionalTakes<V, ByTakes<M>> for Children<C>
where
    Children<C>: Takes<V, M>,
{
    fn take_some(value: V) -> Self {
        <Children<C> as Takes<V, M>>::take(value)
    }
}

// A closure, as `Some`, for the types that take one.

#[diagnostic::do_not_recommend]
impl<H: 'static, C: 'static, R: IntoChild<C>, F: Fn(H) -> R + 'static> OptionalTakes<F, ByClosure>
    for RenderFn<H, C>
{
    fn take_some(value: F) -> Self {
        RenderFn::new(move |handle| value(handle).into_child())
    }
}

#[diagnostic::do_not_recommend]
impl<H, C, R: IntoChild<C>, F: FnOnce(H) -> R + 'static> OptionalTakes<F, ByClosure>
    for Render<H, C>
{
    fn take_some(value: F) -> Self {
        Render::new(move |handle| value(handle).into_child())
    }
}

#[diagnostic::do_not_recommend]
impl<C: 'static, R: IntoChild<C>, F: Fn() -> R + 'static> OptionalTakes<F, ByClosure>
    for BuildFn<C>
{
    fn take_some(value: F) -> Self {
        BuildFn::new(move || value().into_child())
    }
}

#[diagnostic::do_not_recommend]
impl<C, R: IntoChild<C>, F: FnOnce() -> R + 'static> OptionalTakes<F, ByClosure> for Build<C> {
    fn take_some(value: F) -> Self {
        Build::new(move || value().into_child())
    }
}

#[diagnostic::do_not_recommend]
impl<A: 'static, R: 'static, F: Fn(A) -> R + 'static> OptionalTakes<F, ByClosure> for Func<A, R> {
    fn take_some(value: F) -> Self {
        Func::new(value)
    }
}

impl<T> AbsentProp for Option<T> {
    fn absent() -> Self {
        None
    }
}

impl<T> MarkedRequired for Option<T> {}

// String

impl<V, M> IntoPropValue<String, ByTakes<M>> for V
where
    String: Takes<V, M>,
{
    fn into_prop_value(self) -> String {
        <String as Takes<V, M>>::take(self)
    }
}

macro_rules! into_string {
    ($($ty:ty),*) => {$(
        impl Takes<$ty, ByInto> for String {
            fn take(value: $ty) -> String {
                value.into()
            }
        }
    )*};
}

into_string!(
    &str,
    &String,
    &mut str,
    char,
    Box<str>,
    std::borrow::Cow<'_, str>
);

// Callback<V, R>

impl<A, R, V, M> IntoPropValue<Callback<A, R>, ByTakes<M>> for V
where
    Callback<A, R>: Takes<V, M>,
{
    fn into_prop_value(self) -> Callback<A, R> {
        <Callback<A, R> as Takes<V, M>>::take(self)
    }
}

impl<A: 'static, R: 'static, F: FnMut(A) -> R + 'static> Takes<F, ByClosure> for Callback<A, R> {
    fn take(value: F) -> Self {
        Callback::new(value)
    }
}

impl<F: FnMut() + 'static> Takes<F, ByUnitClosure> for Callback<()> {
    fn take(mut value: F) -> Self {
        Callback::new(move |()| value())
    }
}

impl<A: PartialEq + 'static> Takes<Signal<A>, BySignal> for Callback<A> {
    fn take(value: Signal<A>) -> Self {
        Callback::from(value)
    }
}

impl<A: Clone + PartialEq + 'static> Takes<Binding<A>, BySignal> for Callback<A> {
    fn take(value: Binding<A>) -> Self {
        Callback::from(value)
    }
}

impl<V, R> AbsentProp for Callback<V, R> {
    fn absent() -> Self {
        Callback::default()
    }
}

impl<V, R> MarkedRequired for Callback<V, R> {}

// Func<V, R>: a closure.

impl<A: 'static, R: 'static, F: Fn(A) -> R + 'static> IntoPropValue<Func<A, R>, ByClosure> for F {
    fn into_prop_value(self) -> Func<A, R> {
        Func::new(self)
    }
}

// Binding<T>

impl<T, V, M> IntoPropValue<Binding<T>, ByTakes<M>> for V
where
    Binding<T>: Takes<V, M>,
{
    fn into_prop_value(self) -> Binding<T> {
        <Binding<T> as Takes<V, M>>::take(self)
    }
}

impl<T: 'static> Takes<Signal<T>, BySignal> for Binding<T> {
    fn take(value: Signal<T>) -> Self {
        Binding::from(value)
    }
}

impl<T: 'static> Takes<T, ByValue> for Binding<T> {
    fn take(value: T) -> Self {
        Binding::from(create_signal(value))
    }
}

impl Takes<&'static str, ByInto> for Binding<String> {
    fn take(value: &'static str) -> Self {
        Binding::from(create_signal(value.to_string()))
    }
}

/// A binding left out is a signal of its type's default, owned by the component: the macro
/// asks for it inside the component's scope.
impl<T: Default + 'static> AbsentProp for Binding<T> {
    fn absent() -> Self {
        Binding::from(create_signal(T::default()))
    }
}

impl<T> MarkedRequired for Binding<T> {}

// Render<H, C> and RenderFn<H, C>: a closure taking the handle, whose value converts into the
// child. One impl each, so a closure that builds no `C` is reported as that (`IntoChild`).

impl<H: 'static, C: 'static, R: IntoChild<C>, F: Fn(H) -> R + 'static>
    IntoPropValue<RenderFn<H, C>, ByClosure> for F
{
    fn into_prop_value(self) -> RenderFn<H, C> {
        RenderFn::new(move |handle| self(handle).into_child())
    }
}

impl<H, C, R: IntoChild<C>, F: FnOnce(H) -> R + 'static> IntoPropValue<Render<H, C>, ByClosure>
    for F
{
    fn into_prop_value(self) -> Render<H, C> {
        Render::new(move |handle| self(handle).into_child())
    }
}

// Build<C> and BuildFn<C>: a closure taking nothing.

impl<C: 'static, R: IntoChild<C>, F: Fn() -> R + 'static> IntoPropValue<BuildFn<C>, ByClosure>
    for F
{
    fn into_prop_value(self) -> BuildFn<C> {
        BuildFn::new(move || self().into_child())
    }
}

impl<C, R: IntoChild<C>, F: FnOnce() -> R + 'static> IntoPropValue<Build<C>, ByClosure> for F {
    fn into_prop_value(self) -> Build<C> {
        Build::new(move || self().into_child())
    }
}

// Children<C>

impl<C: SlotChild, V, M> IntoPropValue<Children<C>, ByTakes<M>> for V
where
    Children<C>: Takes<V, M>,
{
    fn into_prop_value(self) -> Children<C> {
        <Children<C> as Takes<V, M>>::take(self)
    }
}

impl<C: SlotChild> Takes<Run<C>, ByInto> for Children<C> {
    fn take(value: Run<C>) -> Self {
        value.into()
    }
}

impl<C: SlotChild> Takes<DynamicSegment<C>, ByInto> for Children<C> {
    fn take(value: DynamicSegment<C>) -> Self {
        value.into()
    }
}

impl<C: SlotChild> Takes<ChildSegment<C>, ByInto> for Children<C> {
    fn take(value: ChildSegment<C>) -> Self {
        value.into()
    }
}

impl<C: SlotChild> AbsentProp for Children<C> {
    fn absent() -> Self {
        Children::default()
    }
}

impl<C: SlotChild> MarkedRequired for Children<C> {}

// Trackable: what it converts from.

impl<V, M> IntoPropValue<Trackable, ByTakes<M>> for V
where
    Trackable: Takes<V, M>,
{
    fn into_prop_value(self) -> Trackable {
        <Trackable as Takes<V, M>>::take(self)
    }
}

macro_rules! into_trackable {
    ($($ty:ty),*) => {$(
        impl<T: 'static> Takes<$ty, ByInto> for Trackable {
            fn take(value: $ty) -> Trackable {
                value.into()
            }
        }
    )*};
}

into_trackable!(Signal<T>, ReadSignal<T>, Memo<T>, Reader<T>, Binding<T>);

impl Takes<Trigger, ByInto> for Trackable {
    fn take(value: Trigger) -> Trackable {
        value.into()
    }
}

// Props whose type comes from what is passed.

/// What a prop whose last type argument is one of its component's own type parameters, named
/// by no other prop, takes: what is passed decides that parameter. A conversion that leaves
/// the parameter open (a value into `Prop<V>`, whose `V` could be the value or what it
/// converts into) would leave it unknown, so each type takes here only what names its
/// parameter (`ExactTakes`): itself, a signal, memo or closure for a `Prop` (`IntoReadProp`),
/// a raw closure for a render, a value as `Some` for an `Option<T>`. `P` is the declared type,
/// so every message names it; `K` is `Exact<()>`, or `ExactWithin<()>` for an `Option<X>`
/// whose `X`'s last type argument is the parameter (`Option<RenderFn<H, C>>`), which takes
/// what an exact `X` takes as `Some`. Children written between the tags reach
/// `TakesChildren` instead, which hands them to the slot traits of `Exact<P>` or
/// `ExactWithin<P>`.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be passed as a `{P}` prop",
    label = "this is a `{Self}`"
)]
pub trait IntoExactProp<P, M, K> {
    fn into_exact_prop(self) -> P;
}

/// The kind of an exact prop's slot type (`IntoExactProp`'s `K`).
pub struct Exact<P>(pub P);

pub struct ExactWithin<P>(pub P);

impl<P, K> IntoExactProp<P, Itself, K> for P {
    fn into_exact_prop(self) -> P {
        self
    }
}

impl<P, V, M> IntoExactProp<P, ByExact<M>, Exact<()>> for V
where
    P: ExactTakes<V, M>,
{
    fn into_exact_prop(self) -> P {
        P::take_exact(self)
    }
}

impl<P, V, M> IntoExactProp<P, ByExact<M>, ExactWithin<()>> for V
where
    P: ExactWithinTakes<V, M>,
{
    fn into_exact_prop(self) -> P {
        P::take_within(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as a `{Self}` prop",
    label = "this prop's type comes from what is passed, and this is a `{V}`"
)]
pub trait ExactTakes<V, M>: Sized {
    fn take_exact(value: V) -> Self;
}

#[diagnostic::on_unimplemented(
    message = "a `{V}` cannot be passed as a `{Self}` prop",
    label = "this prop's type comes from what is passed, and this is a `{V}`"
)]
pub trait ExactWithinTakes<V, M>: Sized {
    fn take_within(value: V) -> Self;
}

impl<X: ExactTakes<V, M>, V, M> ExactWithinTakes<V, Given<M>> for Option<X> {
    fn take_within(value: V) -> Self {
        Some(X::take_exact(value))
    }
}

/// A `Prop` takes what `IntoReadProp` does, through `ReadPropTakes`, a trait of
/// `ReadProp<T>`'s, so that a plain value is reported with its message rather than as a value
/// that is no closure. (A trait of `T`'s would not do: `T` is not known yet when it is asked.)
impl<T, V, M> ExactTakes<V, ByProp<M>> for Prop<T>
where
    ReadProp<T>: ReadPropTakes<V, M, Value = T>,
{
    fn take_exact(value: V) -> Self {
        <ReadProp<T> as ReadPropTakes<V, M>>::take_read(value)
    }
}

/// Names a `Prop<T>` read as it changes, for `ReadPropTakes`.
pub struct ReadProp<T>(std::marker::PhantomData<T>);

#[diagnostic::on_unimplemented(
    message = "this prop is read as it changes and its type comes from what is passed, so a `{V}` cannot fill it",
    label = "pass a signal, memo or closure (`move || ..`); for a value fixed when the view is built, write plain Rust such as `shown.then(|| view! {{..}})`",
    note = "it takes a `Prop`, `Signal`, `ReadSignal`, `Memo` or `Binding` of its value, or a closure computing it"
)]
pub trait ReadPropTakes<V, M> {
    type Value;

    fn take_read(value: V) -> Prop<Self::Value>;
}

#[diagnostic::do_not_recommend]
impl<T, V: IntoReadProp<T, M>, M> ReadPropTakes<V, M> for ReadProp<T> {
    type Value = T;

    fn take_read(value: V) -> Prop<T> {
        value.into_read_prop()
    }
}

impl<X> ExactTakes<X, BySome> for Option<X> {
    fn take_exact(value: X) -> Self {
        Some(value)
    }
}

impl<A, R, V, M> ExactTakes<V, ByTakes<M>> for Callback<A, R>
where
    Callback<A, R>: Takes<V, M>,
{
    fn take_exact(value: V) -> Self {
        <Callback<A, R> as Takes<V, M>>::take(value)
    }
}

impl<A: 'static, R: 'static, F: Fn(A) -> R + 'static> ExactTakes<F, ByClosure> for Func<A, R> {
    fn take_exact(value: F) -> Self {
        Func::new(value)
    }
}

impl<T: 'static> ExactTakes<Signal<T>, BySignal> for Binding<T> {
    fn take_exact(value: Signal<T>) -> Self {
        Binding::from(value)
    }
}

impl<C: SlotChild, V, M> ExactTakes<V, ByTakes<M>> for Children<C>
where
    Children<C>: Takes<V, M>,
{
    fn take_exact(value: V) -> Self {
        <Children<C> as Takes<V, M>>::take(value)
    }
}

/// The renders take their own type, or a raw closure: its value is the child as it is.
macro_rules! exact_render {
    ($render:ident, $fn:ident, ($($static:tt)*)) => {

        impl<H $($static)*, C $($static)*, F: $fn(H) -> C + 'static> ExactTakes<F, ByClosure>
            for $render<H, C>
        {
            fn take_exact(value: F) -> Self {
                $render::new(value)
            }
        }
    };
}

exact_render!(RenderFn, Fn, (: 'static));
exact_render!(Render, FnOnce, ());

macro_rules! exact_build {
    ($build:ident, $fn:ident, ($($static:tt)*)) => {

        impl<C $($static)*, F: $fn() -> C + 'static> ExactTakes<F, ByClosure> for $build<C> {
            fn take_exact(value: F) -> Self {
                $build::new(value)
            }
        }
    };
}

exact_build!(BuildFn, Fn, (: 'static));
exact_build!(Build, FnOnce, ());

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

impl<H, C> FnSlot for Slot<ExactWithin<Option<RenderFn<H, C>>>> {
    type Handed = H;
}

impl<H, C> FnOnceSlot for Slot<ExactWithin<Option<Render<H, C>>>> {
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

/// What the children setter asks of the closure `view!` hands it (`Self`), which makes the
/// children `S` written between a tag's tags, wrapped by their shape, for the slot type `P`:
/// `P`'s `TakesBlock`, `TakesClosure`, `TakesUnitClosure` or `TakesWith`, through
/// `ChildShape`, so their diagnostics are the ones shown. `K` is `()`, or for a slot whose type
/// comes from what is passed `Exact<()>` or `ExactWithin<()>` (`IntoExactProp`), whose slot
/// traits are those of `Exact<P>` or `ExactWithin<P>`. Its `Self` is neither the shape nor
/// `P`: rustc reports a failure with the first trait's message when the trait that failed
/// under it has the same `Self`, and names a concrete `Self`'s impls in the message. Its one
/// impl keeps the shape from being inferred from `P` before the children are built.
#[diagnostic::on_unimplemented(
    message = "these children cannot fill this component's children slot",
    label = "these children"
)]
pub trait TakesChildren<P, S, K> {
    fn take_children(children: S) -> P;
}

impl<M, P, S: ChildShape<P, K>, K> TakesChildren<P, S, K> for M {
    fn take_children(children: S) -> P {
        children.into_slot()
    }
}

/// How one shape of children fills a slot of type `P`, through the slot's own trait for it.
pub trait ChildShape<P, K> {
    fn into_slot(self) -> P;
}

macro_rules! slot_shape {
    ($shape:ident, $takes:ident, $take:ident) => {
        impl<P: $takes<F>, F> ChildShape<P, ()> for $shape<F> {
            fn into_slot(self) -> P {
                P::$take(self.0)
            }
        }

        impl<P, F> ChildShape<P, Exact<()>> for $shape<F>
        where
            Exact<P>: $takes<F>,
        {
            fn into_slot(self) -> P {
                <Exact<P> as $takes<F>>::$take(self.0).0
            }
        }

        impl<P, F> ChildShape<P, ExactWithin<()>> for $shape<F>
        where
            ExactWithin<P>: $takes<F>,
        {
            fn into_slot(self) -> P {
                <ExactWithin<P> as $takes<F>>::$take(self.0).0
            }
        }
    };
}

slot_shape!(ChildBlock, TakesBlock, take_block);
slot_shape!(ChildClosure, TakesClosure, take_closure);
slot_shape!(ChildUnitClosure, TakesUnitClosure, take_unit_closure);
slot_shape!(ChildWith, TakesWith, take_with);

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

/// Never implemented: what children written between the tags of a component with no
/// children slot are asked for, so they are reported with this message.
#[diagnostic::on_unimplemented(
    message = "this component takes no children",
    label = "write the tag as `<Tag />`, with nothing between its tags"
)]
pub trait TakesNoChildren {}

/// Never implemented: a slot that hands its children nothing asks it of a closure that names a
/// value, so that closure is reported with this message.
#[diagnostic::on_unimplemented(
    message = "this component hands nothing to its children, so there is no value to name",
    label = "write these children without `|value|`"
)]
pub trait HandsAValue {}

macro_rules! build_slot {
    ($build:ident, $fn:ident, ($($static:tt)*)) => {
        impl<C $($static)*, F: $fn() -> B + 'static, B: OneChild<C>> TakesBlock<F> for $build<C> {
            fn take_block(block: F) -> Self {
                $build::new(move || block().one_child())
            }
        }

        impl<C $($static)*, F: $fn() -> B + 'static, B: IntoChild<C>> TakesUnitClosure<F> for $build<C> {
            fn take_unit_closure(closure: F) -> Self {
                $build::new(move || closure().into_child())
            }
        }

        impl<C $($static)*, F: HandsAValue> TakesClosure<F> for $build<C> {
            fn take_closure(_closure: F) -> Self {
                unreachable!("`HandsAValue` has no impls")
            }
        }

        impl<C $($static)*, F: HandsAValue> TakesWith<F> for $build<C> {
            fn take_with(_closure: F) -> Self {
                unreachable!("`HandsAValue` has no impls")
            }
        }

        impl<C $($static)*, F: $fn() -> C + 'static> TakesBlock<F> for Exact<$build<C>> {
            fn take_block(block: F) -> Self {
                Exact($build::new(block))
            }
        }

        impl<C $($static)*, F: $fn() -> C + 'static> TakesUnitClosure<F> for Exact<$build<C>> {
            fn take_unit_closure(closure: F) -> Self {
                Exact($build::new(closure))
            }
        }

        impl<C $($static)*, F: HandsAValue> TakesClosure<F> for Exact<$build<C>> {
            fn take_closure(_closure: F) -> Self {
                unreachable!("`HandsAValue` has no impls")
            }
        }

        impl<C $($static)*, F: HandsAValue> TakesWith<F> for Exact<$build<C>> {
            fn take_with(_closure: F) -> Self {
                unreachable!("`HandsAValue` has no impls")
            }
        }

        impl<C $($static)*, F> TakesOptionalBlock<F> for $build<C>
        where
            $build<C>: TakesBlock<F>,
        {
            fn take_optional_block(block: F) -> Option<Self> {
                Some($build::take_block(block))
            }
        }
    };
}

build_slot!(BuildFn, Fn, (: 'static));
build_slot!(Build, FnOnce, ());

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

/// A `Children<C>` slot whose `C` comes from what is passed takes a block as any other.
impl<C: SlotChild, F> TakesBlock<F> for Exact<Children<C>>
where
    Children<C>: TakesBlock<F>,
{
    fn take_block(block: F) -> Self {
        Exact(Children::take_block(block))
    }
}

impl<X, F> TakesOptionalBlock<F> for Exact<X>
where
    Exact<X>: TakesBlock<F>,
{
    fn take_optional_block(block: F) -> Option<Self> {
        Some(Self::take_block(block))
    }
}

/// An optional render slot whose child type comes from what is passed: what the exact render
/// takes, as `Some`.
impl<X, F> TakesBlock<F> for ExactWithin<Option<X>>
where
    Exact<X>: TakesOptionalBlock<F>,
{
    fn take_block(block: F) -> Self {
        ExactWithin(Exact::<X>::take_optional_block(block).map(|exact| exact.0))
    }
}

impl<X, F> TakesClosure<F> for ExactWithin<Option<X>>
where
    Exact<X>: TakesClosure<F>,
{
    fn take_closure(closure: F) -> Self {
        ExactWithin(Some(Exact::<X>::take_closure(closure).0))
    }
}

impl<X, F> TakesUnitClosure<F> for ExactWithin<Option<X>>
where
    Exact<X>: TakesUnitClosure<F>,
{
    fn take_unit_closure(closure: F) -> Self {
        ExactWithin(Some(Exact::<X>::take_unit_closure(closure).0))
    }
}

impl<X, F> TakesWith<F> for ExactWithin<Option<X>>
where
    Exact<X>: TakesWith<F>,
{
    fn take_with(closure: F) -> Self {
        ExactWithin(Some(Exact::<X>::take_with(closure).0))
    }
}
