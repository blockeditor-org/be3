use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, Key};
use crate::computation::Computation;
use crate::runtime::{check_tracked_read, probe};
use crate::signal::{Get, Tracked, Value, track};

pub struct Memo<T> {
    key: Key,
    ty: PhantomData<*const T>,
}

pub fn create_memo<T: PartialEq + 'static>(mut compute: impl FnMut() -> T + 'static) -> Memo<T> {
    let probed = probe(true, &mut compute);
    let constant = probed.constant();
    let output = Rc::new(Value::new(Some(probed.value)));
    if constant {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
    } else {
        let written = output.clone();
        let tracked: Rc<dyn Tracked> = output.clone();
        let computation = Computation::new(Some(tracked), move || {
            let next = compute();
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
        key: arena::insert_owned(output),
        ty: PhantomData,
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

impl<T: 'static> Get<T> for Memo<T> {
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        check_tracked_read::<Self>();
        let output = arena::get::<Value<Option<T>>>(self.key)
            .expect("memo used after the scope that owns it was disposed");
        let producer = output.source.producer.borrow().upgrade();
        if let Some(producer) = producer {
            producer.refresh();
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
