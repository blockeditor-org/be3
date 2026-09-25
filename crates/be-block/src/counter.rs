use be_model::{Count, Document, Edit, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Counter {
    pub count: Count,
}

impl Counter {
    pub fn value(&self) -> i64 {
        self.count.get()
    }

    pub fn add(by: i64) -> Edit {
        Self::COUNT.add(ObjectId::ROOT, by).into()
    }

    pub fn reset(&self) -> Edit {
        Self::add(self.value().saturating_neg())
    }
}

impl Root for Counter {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_0001);
}

pub type CounterContent = Document<Counter>;
