use std::{borrow::Cow, ops::Deref};

use crate::{Change, FieldRef, Object, ObjectId, SeqOp, Sequence, Tree, Value, field::Field};

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
