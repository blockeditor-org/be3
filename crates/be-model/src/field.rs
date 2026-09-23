use std::{marker::PhantomData, ops::Deref};

use serde::{Serialize, de::DeserializeOwned};

use crate::{Anchor, Change, Model, Object, ObjectId, Place, Tree, Value};

pub trait Field: Sized {
    fn blank() -> Value;

    fn read(tree: &Tree, value: &Value) -> Self;

    fn write(&self, owner: ObjectId, field: u16, out: &mut Vec<(ObjectId, Object)>) -> Value;
}

pub trait Register: Serialize + DeserializeOwned + Clone + PartialEq + Default {}

impl<T: Serialize + DeserializeOwned + Clone + PartialEq + Default> Register for T {}

pub(crate) fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    postcard::to_stdvec(value).unwrap_or_default()
}

impl<T: Register> Field for T {
    fn blank() -> Value {
        Value::Register(encode(&T::default()))
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Register(bytes) => postcard::from_bytes(bytes).unwrap_or_default(),
            _ => T::default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Register(encode(self))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Count(pub i64);

impl Count {
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl Field for Count {
    fn blank() -> Value {
        Value::Count(0)
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Count(count) => Self(*count),
            _ => Self(0),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Count(self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item<T> {
    pub id: ObjectId,
    pub value: T,
}

impl<T> Deref for Item<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct List<T> {
    items: Vec<Item<T>>,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> List<T> {
    pub fn get(&self, id: ObjectId) -> Option<&Item<T>> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn last_id(&self) -> Option<ObjectId> {
        self.items.last().map(|item| item.id)
    }

    pub fn get_index(&self, index: usize) -> Option<&Item<T>> {
        self.items.get(index)
    }
}

impl<T> Deref for List<T> {
    type Target = [Item<T>];

    fn deref(&self) -> &[Item<T>] {
        &self.items
    }
}

impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        Self {
            items: values
                .into_iter()
                .map(|value| Item {
                    id: ObjectId::new(),
                    value,
                })
                .collect(),
        }
    }
}

impl<T: Model> Field for List<T> {
    fn blank() -> Value {
        Value::List(Vec::new())
    }

    fn read(tree: &Tree, value: &Value) -> Self {
        let Value::List(ids) = value else {
            return Self::default();
        };
        Self {
            items: ids
                .iter()
                .map(|id| Item {
                    id: *id,
                    value: T::read(tree, *id),
                })
                .collect(),
        }
    }

    fn write(&self, owner: ObjectId, field: u16, out: &mut Vec<(ObjectId, Object)>) -> Value {
        let place = Place {
            object: owner,
            field,
        };
        for item in &self.items {
            item.value.write(item.id, Some(place), out);
        }
        Value::List(self.items.iter().map(|item| item.id).collect())
    }
}

pub struct FieldRef<M, F> {
    index: u16,
    marker: PhantomData<fn() -> (M, F)>,
}

impl<M, F> Clone for FieldRef<M, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M, F> Copy for FieldRef<M, F> {}

impl<M, F> FieldRef<M, F> {
    pub const fn new(index: u16) -> Self {
        Self {
            index,
            marker: PhantomData,
        }
    }

    pub const fn index(self) -> u16 {
        self.index
    }

    pub const fn of(self, object: ObjectId) -> Place {
        Place {
            object,
            field: self.index,
        }
    }
}

impl<M, F: Register> FieldRef<M, F> {
    pub fn set(self, object: ObjectId, value: &F) -> Change {
        Change::Set {
            object,
            field: self.index,
            value: encode(value),
        }
    }
}

impl<M> FieldRef<M, Count> {
    pub fn add(self, object: ObjectId, by: i64) -> Change {
        Change::Add {
            object,
            field: self.index,
            by,
        }
    }
}

impl<M, T: Model> FieldRef<M, List<T>> {
    pub fn insert(self, owner: ObjectId, anchor: Anchor, value: &T) -> (ObjectId, Change) {
        let id = ObjectId::new();
        let place = self.of(owner);
        let mut objects = Vec::new();
        value.write(id, Some(place), &mut objects);
        (
            id,
            Change::Insert {
                place,
                anchor,
                objects,
            },
        )
    }

    pub fn move_into(self, owner: ObjectId, anchor: Anchor, object: ObjectId) -> Change {
        Change::Move {
            object,
            place: self.of(owner),
            anchor,
        }
    }
}
