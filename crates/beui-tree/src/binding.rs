use reactive::{Get, Set, Signal, check_tracked_read, create_signal, untrack};

use crate::callback::{Callback, IntoCallback};
use crate::prop::{Getter, IntoProp, Prop};

pub struct Binding<T>(Source<T>);

enum Source<T> {
    Signal(Signal<T>),
    Derived(Getter<T>, Callback<T>),
}

impl<T> Clone for Source<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Source<T> {}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Binding<T> {}

impl<T: Clone + 'static> Binding<T> {
    pub fn new(read: impl IntoProp<T>, write: impl IntoCallback<T, ()>) -> Self {
        let write = write.into_callback();
        match read.into_prop() {
            Prop::Static(value) => {
                let signal = create_signal(value);
                Self(Source::Derived(
                    Getter::of_signal(signal.read_only()),
                    Callback::new(move |value: T| {
                        signal.set_unconditionally(value.clone());
                        write.call(value);
                    }),
                ))
            }
            Prop::Dynamic(read) => Self(Source::Derived(read, write)),
        }
    }

    pub fn map<U: Clone + PartialEq + 'static>(
        self,
        into: impl Fn(T) -> U + 'static,
        from: impl Fn(U) -> T + 'static,
    ) -> Binding<U>
    where
        T: PartialEq,
    {
        Binding::new(
            Prop::dynamic(move || into(self.get())),
            move |value: U| self.set(from(value)),
        )
    }

    pub fn signal(&self) -> Option<Signal<T>> {
        match self.0 {
            Source::Signal(signal) => Some(signal),
            Source::Derived(..) => None,
        }
    }
}

impl<T: 'static> From<Signal<T>> for Binding<T> {
    fn from(signal: Signal<T>) -> Self {
        Self(Source::Signal(signal))
    }
}

impl<T: 'static> Get<T> for Binding<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &self.0 {
            Source::Signal(signal) => signal.with(f),
            Source::Derived(read, _) => {
                check_tracked_read::<Self>();
                f(&read.get())
            }
        }
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &self.0 {
            Source::Signal(signal) => signal.with_untracked(f),
            Source::Derived(read, _) => f(&untrack(|| read.get())),
        }
    }
}

impl<T: Clone + 'static> Set<T> for Binding<T> {
    fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        match &self.0 {
            Source::Signal(signal) => signal.update(f),
            Source::Derived(read, write) => {
                let mut value = untrack(|| read.get());
                let result = f(&mut value);
                write.call(value);
                result
            }
        }
    }

    fn set(&self, value: T)
    where
        T: PartialEq,
    {
        match &self.0 {
            Source::Signal(signal) => signal.set(value),
            Source::Derived(read, write) => {
                if untrack(|| read.get()) != value {
                    write.call(value);
                }
            }
        }
    }
}

pub trait IntoBinding<T> {
    fn into_binding(self) -> Binding<T>;
}

impl<T: 'static> IntoBinding<T> for Binding<T> {
    fn into_binding(self) -> Binding<T> {
        self
    }
}

impl<T: 'static> IntoBinding<T> for Signal<T> {
    fn into_binding(self) -> Binding<T> {
        Binding::from(self)
    }
}

impl<T: 'static> IntoBinding<T> for T {
    fn into_binding(self) -> Binding<T> {
        Binding::from(create_signal(self))
    }
}

impl IntoBinding<String> for &'static str {
    fn into_binding(self) -> Binding<String> {
        Binding::from(create_signal(self.to_string()))
    }
}

impl<T: Clone + 'static> IntoProp<T> for Binding<T> {
    fn into_prop(self) -> Prop<T> {
        match self.0 {
            Source::Signal(signal) => Prop::Dynamic(Getter::of_signal(signal.read_only())),
            Source::Derived(read, _) => Prop::Dynamic(read),
        }
    }
}

impl<T: Clone + PartialEq + 'static> IntoCallback<T, ()> for Binding<T> {
    fn into_callback(self) -> Callback<T> {
        match self.0 {
            Source::Signal(signal) => Callback::from(signal),
            Source::Derived(..) => Callback::new(move |value| self.set(value)),
        }
    }
}
