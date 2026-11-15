use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, Key};
use crate::computation::Computation;
use crate::runtime::{check_tracked_read, probe};
use crate::loading::note_read;
use crate::signal::{Get, Tracked, Value, readiness_of, track};

pub struct Memo<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

/// A cached derived value. It computes once at once, then lazily on read after a
/// dependency changes; an equal output wakes nothing downstream, and chains and diamonds
/// settle before any effect sees them.
///
/// A first run that reads nothing and leaves nothing to clean up makes a constant, with no
/// computation behind it. The closure must be pure: writing a signal in it panics, even
/// under `untrack`, and `./scripts/verify` flags writes in a `create_memo` closure. Nothing
/// registers with a loading `Boundary` from inside a memo, since it reruns on every read.
///
/// The closure may take the value it computed last, `None` the first time: annotate it,
/// `create_memo(move |previous: Option<&Size>| measured.get().or(previous.copied()))`.
pub fn create_memo<T: PartialEq + 'static, M>(
    mut compute: impl Compute<T, M> + 'static,
) -> Memo<T> {
    let probed = probe(true, || compute.compute(None));
    let constant = probed.constant();
    let output = Rc::new(Value::new(Some(probed.value)));
    if constant {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
    } else {
        output.source.set_readiness(readiness_of(
            probed.read.iter().map(|(dependency, _)| dependency.source()),
        ));
        let written = output.clone();
        let tracked: Rc<dyn Tracked> = output.clone();
        let computation = Computation::new(Some(tracked), move || {
            let next = compute.compute(written.value.borrow().as_ref());
            let mut value = written.value.borrow_mut();
            if value.as_ref() == Some(&next) {
                false
            } else {
                *value = Some(next);
                true
            }
        });
        output.source.producer.replace(Rc::downgrade(&computation));
        computation.watch(probed.read, probed.owner);
    }
    Memo {
        key: arena::insert_owned_tracked(output),
        ty: PhantomData,
    }
}

/// What `create_memo` and `create_linked_signal` take: a closure computing the value,
/// `FnMut() -> T`, or one given the value it computed last, `FnMut(Option<&T>) -> T`. `M`
/// only tells the two apart. The argument must be annotated (`|previous: Option<&T>|`): an
/// unannotated `|previous|` fails to infer its type.
pub trait Compute<T, M> {
    fn compute(&mut self, previous: Option<&T>) -> T;
}

#[doc(hidden)]
pub struct Fresh;

#[doc(hidden)]
pub struct FromPrevious;

impl<T, F: FnMut() -> T> Compute<T, Fresh> for F {
    fn compute(&mut self, _: Option<&T>) -> T {
        self()
    }
}

impl<T, F: FnMut(Option<&T>) -> T> Compute<T, FromPrevious> for F {
    fn compute(&mut self, previous: Option<&T>) -> T {
        self(previous)
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Memo<T> {}

impl<T> PartialEq for Memo<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for Memo<T> {}

impl<T> std::fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Memo").field(&self.key).finish()
    }
}

impl<T: 'static> Get<T> for Memo<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        let output = arena::get::<Value<Option<T>>>(self.key)
            .expect("memo used after the scope that owns it was disposed");
        let producer = output.source.producer.borrow().upgrade();
        if let Some(producer) = producer {
            producer.refresh();
        }
        if !output.source.is_settled() {
            note_read(output.clone());
        }
        track(output.clone());
        f(output.value.borrow().as_ref().expect("memo has no value"))
    }
}

impl<T: 'static> Memo<T> {
    pub fn is_alive(&self) -> bool {
        arena::get::<Value<Option<T>>>(self.key).is_some()
    }
}
