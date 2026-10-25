use std::ops::Deref;

use crate::field::{Field, Register};
use crate::stored::{decode, encode};
use crate::{Change, FieldRef, Object, ObjectId, Pos, SeqOp, Sequence, Shape, Tree, Value, schema};

#[derive(Clone, Debug)]
pub struct Series<T> {
    items: Vec<T>,
    read_from: Vec<(Pos, u64)>,
}

impl<T: PartialEq> PartialEq for Series<T> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<T: Eq> Eq for Series<T> {}

impl<T> Default for Series<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T> Series<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            read_from: Vec::new(),
        }
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

    fn read(tree: &Tree, value: &Value) -> Self {
        Self::default().reread(tree, value)
    }

    fn reread(mut self, _tree: &Tree, value: &Value) -> Self {
        let Value::Series(sequence) = value else {
            return Self::default();
        };
        let fragments: Vec<(Pos, u64)> = sequence
            .runs()
            .filter(|run| run.visible)
            .map(|run| (run.first, run.len))
            .collect();
        let kept = match self.read_from.split_last() {
            Some((&(last, len), before))
                if fragments.len() >= self.read_from.len()
                    && fragments[..before.len()] == *before
                    && fragments[before.len()].0 == last
                    && fragments[before.len()].1 >= len =>
            {
                self.items.len()
            }
            _ => 0,
        };
        self.items.truncate(kept);
        let mut at = 0;
        for run in sequence.runs().filter(|run| run.visible) {
            let end = at + run.items.len();
            if end > kept {
                let skip = kept.saturating_sub(at);
                self.items.extend(
                    run.items[skip..]
                        .iter()
                        .map(|bytes| decode(bytes).unwrap_or_default()),
                );
            }
            at = end;
        }
        self.read_from = fragments;
        self
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

    pub fn append(self, object: ObjectId, held: &Sequence<Vec<u8>>, items: &[T]) -> Option<Change> {
        let op = held.insert(
            crate::local_client(),
            held.len(),
            items.iter().map(encode).collect(),
        )?;
        Some(self.edit(object, op))
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
