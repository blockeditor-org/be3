use reactive::{
    Get, Memo, ReadSignal, Ready, Signal, Stored, check_tracked_read, hold, store, untrack,
};

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

impl<T: 'static> IntoProp<Option<T>> for T {
    fn into_prop(self) -> Prop<Option<T>> {
        Prop::Static(Some(self))
    }
}

impl IntoProp<Option<String>> for &'static str {
    fn into_prop(self) -> Prop<Option<String>> {
        Prop::Static(Some(self.to_string()))
    }
}

impl<T: 'static> IntoProp<Option<T>> for Prop<T> {
    fn into_prop(self) -> Prop<Option<T>> {
        self.map(Some)
    }
}

impl<T: Clone + 'static> IntoProp<Option<T>> for ReadSignal<T> {
    fn into_prop(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self.get()))
    }
}

impl<T: Clone + 'static> IntoProp<Option<T>> for Signal<T> {
    fn into_prop(self) -> Prop<Option<T>> {
        Prop::dynamic(move || Some(self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<Option<T>> for Memo<T> {
    fn into_prop(self) -> Prop<Option<T>> {
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
