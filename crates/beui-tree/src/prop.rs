use beui_reactive::{
    Get, Memo, ReadSignal, Ready, Signal, Stored, check_tracked_read, hold, store, untrack,
};

/// A value or a getter of one. Reads through `Get` like a signal, so a component reading
/// its own prop does it inside a memo or effect, not in its body. Pass a memo as it is
/// rather than `move || memo.get()`, which boxes a closure and wakes on every read.
pub enum Prop<T> {
    Static(T),
    Dynamic(Getter<T>),
}

impl<T: Clone> Clone for Prop<T> {
    fn clone(&self) -> Self {
        match self {
            Prop::Static(value) => Prop::Static(value.clone()),
            Prop::Dynamic(read) => Prop::Dynamic(*read),
        }
    }
}

impl<T: Copy> Copy for Prop<T> {}

pub struct Getter<T>(Source<T>);

enum Source<T> {
    Function(Stored<Box<dyn Fn() -> T>>),
    Signal(ReadSignal<T>, fn(&ReadSignal<T>) -> T),
    Memo(Memo<T>, fn(&Memo<T>) -> T),
}

impl<T> Clone for Source<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Source<T> {}

impl<T> Clone for Getter<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Getter<T> {}

impl<T: 'static> Getter<T> {
    pub fn new(read: impl Fn() -> T + 'static) -> Self {
        Self(Source::Function(store(Box::new(read))))
    }

    pub fn of_signal(signal: ReadSignal<T>) -> Self
    where
        T: Clone,
    {
        Self(Source::Signal(signal, <ReadSignal<T> as Get<T>>::get))
    }

    pub fn of_memo(memo: Memo<T>) -> Self
    where
        T: Clone + PartialEq,
    {
        Self(Source::Memo(memo, <Memo<T> as Get<T>>::get))
    }

    pub fn get(&self) -> T {
        match &self.0 {
            Source::Function(read) => read.with(|read| read()),
            Source::Signal(signal, read) => read(signal),
            Source::Memo(memo, read) => read(memo),
        }
    }

    pub fn is_alive(&self) -> bool {
        match &self.0 {
            Source::Function(read) => read.try_with(|_| ()).is_some(),
            Source::Signal(signal, _) => signal.is_alive(),
            Source::Memo(memo, _) => memo.is_alive(),
        }
    }
}

impl<T: 'static> Prop<T> {
    pub fn dynamic(read: impl Fn() -> T + 'static) -> Self {
        Prop::Dynamic(Getter::new(read))
    }

    pub fn map<U: 'static>(self, f: impl Fn(T) -> U + 'static) -> Prop<U> {
        match self {
            Prop::Static(value) => Prop::Static(f(value)),
            Prop::Dynamic(read) => Prop::dynamic(move || f(read.get())),
        }
    }

    pub fn is_alive(&self) -> bool {
        match self {
            Prop::Static(_) => true,
            Prop::Dynamic(read) => read.is_alive(),
        }
    }
}

impl<T: 'static> Get<T> for Prop<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        match self {
            Prop::Static(value) => f(value),
            Prop::Dynamic(read) => f(&read.get()),
        }
    }

    fn get(&self) -> T
    where
        T: Clone,
    {
        check_tracked_read::<Self>();
        match self {
            Prop::Static(value) => value.clone(),
            Prop::Dynamic(read) => read.get(),
        }
    }

    fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        untrack(|| self.get())
    }
}

pub trait IntoProp<T> {
    fn into_prop(self) -> Prop<T>;
}

/// How a `#[component]` prop of type `Prop<T>` takes what a tag passes it: anything
/// `IntoProp<T>` (a value, a signal or memo of one), or a closure computing one, which the
/// prop reads as a getter: `condition={move || !hex.get()}`. `M` only tells the two apart.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be passed as a `{T}` prop",
    label = "pass a `{T}`, a signal or memo of one, or a closure returning one"
)]
pub trait IntoPropWith<T, M> {
    fn into_prop_with(self) -> Prop<T>;
}

pub struct ByValue;
pub struct ByClosure;
pub struct ByLift;
pub struct ByClosureLifted;

/// The markers that pass a value as the prop's own type, without lifting it into an `Option`.
/// A `Prop<V>` whose `V` is a type parameter of its component takes only these, since its type
/// comes from the other props: `Dynamic` and `Keyed` with a `fallback` take an `Option`.
#[diagnostic::on_unimplemented(
    message = "this prop's type comes from the component's other props, so a value is not lifted into an `Option`: pass `Some(..)` (for a `Dynamic` or `Keyed`, a `fallback` makes `value` an `Option`)",
    label = "this is not an `Option`"
)]
pub trait Exact {}
impl Exact for ByValue {}
impl Exact for ByClosure {}

/// A value, signal or memo of `T` passed where a `Prop<Option<T>>` is taken: it is `Some`.
pub trait IntoLiftedProp<T> {
    fn into_lifted_prop(self) -> Prop<T>;
}

impl<T, V: IntoLiftedProp<T>> IntoPropWith<T, ByLift> for V {
    fn into_prop_with(self) -> Prop<T> {
        self.into_lifted_prop()
    }
}

impl<T, V: IntoProp<T>> IntoPropWith<T, ByValue> for V {
    fn into_prop_with(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoPropWith<T, ByClosure> for F {
    fn into_prop_with(self) -> Prop<T> {
        Prop::dynamic(self)
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoPropWith<Option<T>, ByClosureLifted> for F {
    fn into_prop_with(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self()))
    }
}

impl<T: 'static> IntoProp<T> for T {
    fn into_prop(self) -> Prop<T> {
        Prop::Static(self)
    }
}

impl IntoProp<String> for &'static str {
    fn into_prop(self) -> Prop<String> {
        Prop::Static(self.to_string())
    }
}

impl<T: 'static> IntoProp<T> for Prop<T> {
    fn into_prop(self) -> Prop<T> {
        self
    }
}

impl<T: Clone + 'static> IntoProp<T> for ReadSignal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Getter::of_signal(self))
    }
}

impl<T: Clone + 'static> IntoProp<T> for Signal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Getter::of_signal(self.read_only()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Getter::of_memo(self))
    }
}

impl<T: Clone + PartialEq + Default + 'static> IntoProp<T> for Memo<Ready<T>> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Getter::of_memo(hold(self)))
    }
}

impl<T: 'static> IntoLiftedProp<Option<T>> for T {
    fn into_lifted_prop(self) -> Prop<Option<T>> {
        Prop::Static(Some(self))
    }
}

impl IntoLiftedProp<Option<String>> for &'static str {
    fn into_lifted_prop(self) -> Prop<Option<String>> {
        Prop::Static(Some(self.to_string()))
    }
}

impl<T: 'static> IntoLiftedProp<Option<T>> for Prop<T> {
    fn into_lifted_prop(self) -> Prop<Option<T>> {
        self.map(Some)
    }
}

impl<T: Clone + 'static> IntoLiftedProp<Option<T>> for ReadSignal<T> {
    fn into_lifted_prop(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self.get()))
    }
}

impl<T: Clone + 'static> IntoLiftedProp<Option<T>> for Signal<T> {
    fn into_lifted_prop(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoLiftedProp<Option<T>> for Memo<T> {
    fn into_lifted_prop(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self.get()))
    }
}

pub struct Func<V, R>(Stored<Box<dyn Fn(V) -> R>>);

impl<V: 'static, R: 'static> Func<V, R> {
    pub fn new(function: impl Fn(V) -> R + 'static) -> Self {
        Self(store(Box::new(function)))
    }

    pub fn call(&self, value: V) -> R {
        self.0.with(|function| function(value))
    }
}

impl<V, R> Clone for Func<V, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V, R> Copy for Func<V, R> {}

pub trait IntoFunc<V, R> {
    fn into_func(self) -> Func<V, R>;
}

impl<V: 'static, R: 'static, F: Fn(V) -> R + 'static> IntoFunc<V, R> for F {
    fn into_func(self) -> Func<V, R> {
        Func::new(self)
    }
}

impl<V, R> IntoFunc<V, R> for Func<V, R> {
    fn into_func(self) -> Func<V, R> {
        self
    }
}

#[macro_export]
macro_rules! into_prop_via {
    ($target:ty, $value:ty, $convert:expr) => {
        impl $crate::reactive::IntoProp<$target> for $value {
            fn into_prop(self) -> $crate::reactive::Prop<$target> {
                $crate::reactive::Prop::Static(self).map($convert)
            }
        }

        impl $crate::reactive::IntoProp<$target> for $crate::reactive::Prop<$value> {
            fn into_prop(self) -> $crate::reactive::Prop<$target> {
                self.map($convert)
            }
        }

        impl $crate::reactive::IntoProp<$target> for $crate::reactive::Memo<$value> {
            fn into_prop(self) -> $crate::reactive::Prop<$target> {
                $crate::reactive::IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl $crate::reactive::IntoProp<$target> for $crate::reactive::ReadSignal<$value> {
            fn into_prop(self) -> $crate::reactive::Prop<$target> {
                $crate::reactive::IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl $crate::reactive::IntoProp<$target> for $crate::reactive::Signal<$value> {
            fn into_prop(self) -> $crate::reactive::Prop<$target> {
                $crate::reactive::IntoProp::<$value>::into_prop(self).map($convert)
            }
        }
    };
}
