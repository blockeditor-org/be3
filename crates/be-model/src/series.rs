use std::ops::Deref;

use crate::field::{Field, Register};
use crate::stored::{decode, encode};
use crate::{Change, FieldRef, Object, ObjectId, SeqOp, Sequence, Shape, Tree, Value, schema};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Series<T> {
    items: Vec<T>,
}

impl<T> Default for Series<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Series<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }

    pub fn into_vec(self) -> Vec<T> {
        self.items
    }
}

impl<T> From<Vec<T>> for Series<T> {
    fn from(items: Vec<T>) -> Self {
        Self::new(items)
    }
}

impl<T> FromIterator<T> for Series<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        Self::new(items.into_iter().collect())
    }
}

impl<T> Deref for Series<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T: Register> Field for Series<T> {
    fn blank() -> Value {
        Value::Series(Sequence::default())
    }

    fn shape() -> Shape {
        Shape::Series {
            accepts: schema::accepts::<T>,
        }
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Series(sequence) => sequence
                .iter()
                .map(|bytes| decode(bytes).unwrap_or_default())
                .collect(),
            _ => Self::default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Series(Sequence::from_items(
            self.items.iter().map(encode).collect(),
        ))
    }
}

impl<M, T: Register> FieldRef<M, Series<T>> {
    pub fn edit(self, object: ObjectId, op: SeqOp<Vec<u8>>) -> Change {
        Change::Series {
            object,
            field: self.index(),
            op,
        }
    }

    pub fn update(
        self,
        object: ObjectId,
        held: &Sequence<Vec<u8>>,
        wanted: &[T],
    ) -> Option<Change> {
        let wanted: Vec<Vec<u8>> = wanted.iter().map(encode).collect();
        let kept = held
            .iter()
            .zip(&wanted)
            .take_while(|(held, wanted)| held == wanted)
            .count();
        let client = crate::local_client();
        let op = match (kept == held.len(), kept == wanted.len()) {
            (true, true) => return None,
            (true, false) => held.insert(client, kept, wanted[kept..].to_vec()),
            (false, true) => held.delete(kept..held.len()),
            (false, false) => held.replace(client, kept..held.len(), wanted[kept..].to_vec()),
        }?;
        Some(self.edit(object, op))
    }
}
