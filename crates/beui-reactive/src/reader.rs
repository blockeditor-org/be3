use crate::async_memo::AsyncMemo;
use crate::loading::Ready;
use crate::memo::Memo;
use crate::runtime::check_tracked_read;
use crate::signal::{Get, ReadSignal, Signal};
use crate::source::Source;
use crate::stored::{Stored, store};

/// Any read-only reactive value of a `T`: a signal, a memo, or a closure computing one. An
/// API that only reads takes a `Reader<T>` (or `impl Into<Reader<T>>`), so its caller passes
/// whichever it has. `Copy`, like every handle; reading borrows a signal's or memo's value
/// rather than cloning it.
pub struct Reader<T>(Held<T>);

enum Held<T> {
    Signal(ReadSignal<T>),
    /// A `Source` or async memo, read in place through `Get`: `with` gives it a closure to
    /// call once with the value.
    // A lent read, given a closure it calls once with the value, as the comment above says.
    #[allow(clippy::type_complexity)]
    Lent(Stored<Box<dyn Fn(&mut dyn FnMut(&T))>>),
    Memo(Memo<T>),
    Function(Stored<Box<dyn Fn() -> T>>),
}

impl<T> Clone for Held<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Held<T> {}

impl<T> Clone for Reader<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Reader<T> {}

impl<T: 'static> Reader<T> {
    /// A reader computing its value on every read, as a closure prop does. Make a memo
    /// instead when the result changes less often than what it reads.
    pub fn new(read: impl Fn() -> T + 'static) -> Self {
        Self(Held::Function(store(Box::new(read))))
    }

    pub fn is_alive(&self) -> bool {
        match &self.0 {
            Held::Signal(signal) => signal.is_alive(),
            Held::Memo(memo) => memo.is_alive(),
            Held::Function(read) => read.is_alive(),
            Held::Lent(read) => read.is_alive(),
        }
    }
}

impl<T: 'static> Get<T> for Reader<T> {
    #[track_caller]
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        match &self.0 {
            Held::Signal(signal) => signal.with(f),
            Held::Memo(memo) => memo.with(f),
            Held::Function(read) => read.with(|read| f(&read())),
            Held::Lent(read) => {
                let mut f = Some(f);
                let mut result = None;
                read.with(|read| {
                    read(&mut |value| {
                        if let Some(f) = f.take() {
                            result = Some(f(value));
                        }
                    })
                });
                result.expect("a lent reader calls back once")
            }
        }
    }

    /// A closure's value as it returns it, without the clone `with` would need.
    #[track_caller]
    fn get(&self) -> T
    where
        T: Clone,
    {
        match &self.0 {
            Held::Function(read) => {
                check_tracked_read::<Self>();
                read.with(|read| read())
            }
            _ => self.with(T::clone),
        }
    }
}

impl<T: 'static> From<ReadSignal<T>> for Reader<T> {
    fn from(signal: ReadSignal<T>) -> Self {
        Self(Held::Signal(signal))
    }
}

impl<T: 'static> From<Signal<T>> for Reader<T> {
    fn from(signal: Signal<T>) -> Self {
        Self(Held::Signal(signal.read_only()))
    }
}

impl<T: 'static> From<Memo<T>> for Reader<T> {
    fn from(memo: Memo<T>) -> Self {
        Self(Held::Memo(memo))
    }
}

impl<T: 'static> From<Source<T>> for Reader<Ready<T>> {
    fn from(source: Source<T>) -> Self {
        Self(Held::Lent(store(Box::new(
            move |f: &mut dyn FnMut(&Ready<T>)| source.with(|value| f(value)),
        ))))
    }
}

impl<T: 'static> From<AsyncMemo<T>> for Reader<Ready<T>> {
    fn from(memo: AsyncMemo<T>) -> Self {
        Self(Held::Lent(store(Box::new(
            move |f: &mut dyn FnMut(&Ready<T>)| memo.with(|value| f(value)),
        ))))
    }
}
