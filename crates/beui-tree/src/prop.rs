use beui_reactive::{
    Get, Memo, ReadSignal, Reader, Ready, Signal, Stored, check_tracked_read, hold, store,
};

/// A value or a getter of one. Reads through `Get` like a signal, so a component reading
/// its own prop does it inside a memo or effect, not in its body. Pass a memo as it is
/// rather than `move || memo.get()`, which boxes a closure and wakes on every read.
pub enum Prop<T> {
    Static(T),
    Dynamic(Reader<T>),
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

impl<T: 'static> Prop<T> {
    pub fn dynamic(read: impl Fn() -> T + 'static) -> Self {
        Prop::Dynamic(Reader::new(read))
    }

    pub fn map<U: 'static>(self, f: impl Fn(T) -> U + 'static) -> Prop<U>
    where
        T: Clone,
    {
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
            Prop::Dynamic(read) => read.with(f),
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
}

pub trait IntoProp<T> {
    fn into_prop(self) -> Prop<T>;
}

/// What a `Prop<T>` takes besides a `Prop<T>`: anything `IntoProp<T>` (a value, a signal or
/// memo of one), or a closure computing one, which the prop reads as a getter:
/// `condition={move || !hex.get()}`. `M` only tells the impls apart. Code outside a tag asks
/// for `IntoPropValue<Prop<T>, M>`, which takes a `Prop<T>` too.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be passed as a `{T}` prop",
    label = "pass a `{T}`, a signal or memo of one, or a closure returning one"
)]
pub trait IntoPropFrom<T, M> {
    fn into_prop_from(self) -> Prop<T>;
}

pub struct ByValue;
pub struct ByClosure;
pub struct ByLift;
pub struct ByClosureLifted;

/// Marks an `Option<X>` prop given what `X` takes (`IntoPropValue`).
pub struct Given<M>(std::marker::PhantomData<M>);

/// How a `Prop<V>` whose `V` is a type parameter of its own component (`Show`'s, `Dynamic`'s
/// and `Keyed`'s `value`) takes what a tag passes it besides a `Prop<V>`, which every type
/// takes as itself: a signal, memo or binding of the value, or a closure computing it. The value's type comes from what is passed, so a plain value, which
/// every type would convert from, is not taken: a value fixed when the view is built is
/// plain Rust (`shown.then(|| view! {..})`).
#[diagnostic::on_unimplemented(
    message = "this prop is read as it changes and its type comes from what is passed, so a `{Self}` cannot fill it",
    label = "pass a signal, memo or closure (`move || ..`); for a value fixed when the view is built, write plain Rust such as `shown.then(|| view! {{..}})`"
)]
pub trait IntoReadProp<T, M> {
    fn into_read_prop(self) -> Prop<T>;
}

impl<T: Clone + 'static> IntoReadProp<T, ByValue> for ReadSignal<T> {
    fn into_read_prop(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: Clone + 'static> IntoReadProp<T, ByValue> for Signal<T> {
    fn into_read_prop(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: Clone + PartialEq + 'static> IntoReadProp<T, ByValue> for Memo<T> {
    fn into_read_prop(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoReadProp<T, ByClosure> for F {
    fn into_read_prop(self) -> Prop<T> {
        Prop::dynamic(self)
    }
}

/// A value, signal or memo of `T` passed where a `Prop<Option<T>>` is taken: it is `Some`.
pub trait IntoLiftedProp<T> {
    fn into_lifted_prop(self) -> Prop<T>;
}

impl<T, V: IntoLiftedProp<T>> IntoPropFrom<T, ByLift> for V {
    fn into_prop_from(self) -> Prop<T> {
        self.into_lifted_prop()
    }
}

impl<T, V: IntoProp<T>> IntoPropFrom<T, ByValue> for V {
    fn into_prop_from(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoPropFrom<T, ByClosure> for F {
    fn into_prop_from(self) -> Prop<T> {
        Prop::dynamic(self)
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoPropFrom<Option<T>, ByClosureLifted> for F {
    fn into_prop_from(self) -> Prop<Option<T>> {
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

impl<T: Clone + 'static> IntoProp<T> for ReadSignal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(self.into())
    }
}

impl<T: Clone + 'static> IntoProp<T> for Signal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(self.into())
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(self.into())
    }
}

impl<T: Clone + PartialEq + Default + 'static> IntoProp<T> for Memo<Ready<T>> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(hold(self).into())
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

impl<T: Clone + 'static> IntoLiftedProp<Option<T>> for Prop<T> {
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
