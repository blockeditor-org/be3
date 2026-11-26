use beui_reactive::{Get, Reader, Set, Signal, Trackable, create_signal};

use crate::callback::Callback;
use crate::prop::{IntoProp, Prop};
use crate::prop_value::IntoPropValue;

/// State a component both shows and changes (a toggle's `checked`), instead of a prop and an
/// `on_change`. The caller picks the owner: leave it out (the component's own signal), pass
/// a `Signal<T>` (no allocation), a plain value (seeds a signal the component owns), or
/// `Binding::new(read, write)` for derived state. With `new` the component shows what
/// `read` says, so a write the callback ignores changes nothing, and `set` skips a write
/// equal to what `read` gives. A binding also fills a `Prop<T>` or a `Callback<T>` prop.
pub struct Binding<T>(Source<T>);

enum Source<T> {
    Signal(Signal<T>),
    Derived(Reader<T>, Callback<T>),
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
    pub fn new<M, W>(
        read: impl IntoPropValue<Prop<T>, M>,
        write: impl IntoPropValue<Callback<T>, W>,
    ) -> Self {
        let write = write.into_prop_value();
        match read.into_prop_value() {
            Prop::Static(value) => {
                let signal = create_signal(value);
                Self(Source::Derived(
                    signal.into(),
                    Callback::new(move |value: T| {
                        signal.set_unconditionally(value.clone());
                        write.call(value);
                    }),
                ))
            }
            Prop::Dynamic(read) => Self(Source::Derived(read, write)),
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
            Source::Derived(read, _) => read.with(f),
        }
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &self.0 {
            Source::Signal(signal) => signal.with_untracked(f),
            Source::Derived(read, _) => read.with_untracked(f),
        }
    }
}

impl<T: Clone + 'static> Set<T> for Binding<T> {
    fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        match &self.0 {
            Source::Signal(signal) => signal.update(f),
            Source::Derived(read, write) => {
                let mut value = read.get_untracked();
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
                if read.get_untracked() != value {
                    write.call(value);
                }
            }
        }
    }
}

impl<T: Clone + 'static> crate::prop::IntoReadProp<T, crate::prop::ByValue> for Binding<T> {
    fn into_read_prop(self) -> Prop<T> {
        self.into_prop()
    }
}

impl<T: Clone + 'static> IntoProp<T> for Binding<T> {
    fn into_prop(self) -> Prop<T> {
        match self.0 {
            Source::Signal(signal) => Prop::Dynamic(signal.into()),
            Source::Derived(read, _) => Prop::Dynamic(read),
        }
    }
}

impl<T: Clone + PartialEq + 'static> From<Binding<T>> for Callback<T> {
    fn from(binding: Binding<T>) -> Self {
        match binding.0 {
            Source::Signal(signal) => Callback::from(signal),
            Source::Derived(..) => Callback::new(move |value| binding.set(value)),
        }
    }
}

impl<T: 'static> From<Binding<T>> for Trackable {
    fn from(binding: Binding<T>) -> Self {
        Trackable::new(move || binding.track())
    }
}

impl<T: 'static> From<Binding<T>> for Option<Trackable> {
    fn from(binding: Binding<T>) -> Self {
        Some(binding.into())
    }
}
