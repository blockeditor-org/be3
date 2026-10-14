use reactive::{Memo, ReadSignal, Stored, store, untrack};

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

pub struct Getter<T>(Stored<Box<dyn Fn() -> T>>);

impl<T> Clone for Getter<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Getter<T> {}

impl<T: 'static> Getter<T> {
    pub fn new(read: impl Fn() -> T + 'static) -> Self {
        Self(store(Box::new(read)))
    }

    pub fn get(&self) -> T {
        self.0.with(|read| read())
    }
}

impl<T: 'static> Prop<T> {
    pub fn dynamic(read: impl Fn() -> T + 'static) -> Self {
        Prop::Dynamic(Getter::new(read))
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        match self {
            Prop::Static(value) => value.clone(),
            Prop::Dynamic(read) => read.get(),
        }
    }

    pub fn peek(&self) -> T
    where
        T: Clone,
    {
        untrack(|| self.get())
    }

    pub fn map<U: 'static>(self, f: impl Fn(T) -> U + 'static) -> Prop<U> {
        match self {
            Prop::Static(value) => Prop::Static(f(value)),
            Prop::Dynamic(read) => Prop::dynamic(move || f(read.get())),
        }
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
        Prop::dynamic(move || self.get())
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::dynamic(move || self.get())
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
