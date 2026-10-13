use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, Key};
use crate::computation::Computation;
use crate::runtime::untrack;
use crate::signal::{Source, Value};

struct Inner<T> {
    output: Rc<Value<Option<T>>>,
    computation: Rc<Computation>,
}

pub struct Memo<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

pub fn create_memo<T: PartialEq + 'static>(mut compute: impl FnMut() -> T + 'static) -> Memo<T> {
    let source = Rc::new(Source::default());
    let output = Rc::new(Value {
        value: RefCell::new(None),
        source: source.clone(),
    });
    let written = output.clone();
    let computation = Computation::new(
        Some(source.clone()),
        Box::new(move || {
            let next = compute();
            let mut value = written.value.borrow_mut();
            if value.as_ref() == Some(&next) {
                false
            } else {
                *value = Some(next);
                true
            }
        }),
    );
    source.producer.replace(Rc::downgrade(&computation));
    computation.refresh();
    let key = arena::insert_owned(Rc::new(Inner {
        output,
        computation,
    }));
    Memo {
        key,
        ty: PhantomData,
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Memo<T> {}

impl<T: 'static> Memo<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let inner = arena::get::<Inner<T>>(self.key)
            .expect("memo used after the scope that owns it was disposed");
        inner.computation.refresh();
        inner.output.source.track();
        f(inner
            .output
            .value
            .borrow()
            .as_ref()
            .expect("memo has no value"))
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        untrack(|| self.get())
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        untrack(|| self.with(f))
    }

    pub fn is_alive(&self) -> bool {
        arena::get::<Inner<T>>(self.key).is_some()
    }
}
