use std::{borrow::Cow, ops::Deref};

use sequence::marks::{Mark, MarkId, Rich, Seed};

use crate::{
    Change, FieldRef, Object, ObjectId, SeqOp, Sequence, Shape, Tree, Value, field::Field,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Text {
    bytes: Vec<u8>,
}

impl Text {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn to_str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        Self::new(text.as_bytes())
    }
}

impl Deref for Text {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Field for Text {
    fn blank() -> Value {
        Value::Text(Sequence::default())
    }

    fn shape() -> Shape {
        Shape::Text
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Text(sequence) => Self::new(sequence.items()),
            _ => Self::default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Text(Sequence::from_items(self.bytes.clone()))
    }
}

impl<M> FieldRef<M, Text> {
    pub fn edit(self, object: ObjectId, op: SeqOp<u8>) -> Change {
        Change::Text {
            object,
            field: self.index(),
            op,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RichText {
    bytes: Vec<u8>,
    seeds: Vec<Seed>,
}

impl RichText {
    pub fn new(bytes: impl Into<Vec<u8>>, seeds: Vec<Seed>) -> Self {
        let rich = Rich::load(bytes.into(), &seeds);
        Self {
            seeds: rich.seeds(),
            bytes: rich.text.items(),
        }
    }

    pub fn plain(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            seeds: Vec::new(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn seeds(&self) -> &[Seed] {
        &self.seeds
    }

    pub fn to_str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

impl From<&str> for RichText {
    fn from(text: &str) -> Self {
        Self::plain(text.as_bytes())
    }
}

impl Deref for RichText {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Field for RichText {
    fn blank() -> Value {
        Value::Rich(Rich::default())
    }

    fn shape() -> Shape {
        Shape::Rich
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Rich(rich) => Self {
                bytes: rich.text.items(),
                seeds: rich.seeds(),
            },
            _ => Self::default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Rich(Rich::load(self.bytes.clone(), &self.seeds))
    }
}

impl<M> FieldRef<M, RichText> {
    pub fn edit(self, object: ObjectId, op: SeqOp<u8>) -> Change {
        Change::Text {
            object,
            field: self.index(),
            op,
        }
    }

    pub fn mark(self, object: ObjectId, mark: Mark) -> Change {
        Change::Mark {
            object,
            field: self.index(),
            mark,
        }
    }

    pub fn unmark(self, object: ObjectId, id: MarkId) -> Change {
        Change::Unmark {
            object,
            field: self.index(),
            id,
        }
    }
}
