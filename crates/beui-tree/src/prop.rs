use std::rc::Rc;

use reactive::{Memo, ReadSignal, untrack};

pub enum Prop<T> {
    Static(T),
    Dynamic(Rc<dyn Fn() -> T>),
}

impl<T: Clone> Clone for Prop<T> {
    fn clone(&self) -> Self {
        match self {
            Prop::Static(value) => Prop::Static(value.clone()),
            Prop::Dynamic(read) => Prop::Dynamic(Rc::clone(read)),
        }
    }
}

impl<T: 'static> Prop<T> {
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        match self {
            Prop::Static(value) => value.clone(),
            Prop::Dynamic(read) => read(),
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
            Prop::Dynamic(read) => Prop::Dynamic(Rc::new(move || f(read()))),
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
        Prop::Dynamic(Rc::new(move || self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Rc::new(move || self.get()))
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
        Prop::Dynamic(Rc::new(move || Some(self.get())))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<Option<T>> for Memo<T> {
    fn into_prop(self) -> Prop<Option<T>> {
        Prop::Dynamic(Rc::new(move || Some(self.get())))
    }
}

pub struct Func<V, R>(Rc<dyn Fn(V) -> R>);

impl<V, R> Func<V, R> {
    pub fn new(function: impl Fn(V) -> R + 'static) -> Self {
        Self(Rc::new(function))
    }

    pub fn call(&self, value: V) -> R {
        (self.0)(value)
    }
}

impl<V, R> Clone for Func<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub trait IntoFunc<V, R> {
    fn into_func(self) -> Func<V, R>;
}

impl<V, R, F: Fn(V) -> R + 'static> IntoFunc<V, R> for F {
    fn into_func(self) -> Func<V, R> {
        Func::new(self)
    }
}

impl<V, R> IntoFunc<V, R> for Func<V, R> {
    fn into_func(self) -> Func<V, R> {
        self
    }
}
