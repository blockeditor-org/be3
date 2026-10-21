use serde::{Deserialize, Serialize};

pub use sequence::marks::*;

use crate::SeqOp;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    pub scope: Scope,
    pub expand: Expand,
    pub key: Name,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RichOp {
    Text(SeqOp<u8>),
    Format(FormatOp),
    Unformat(MarkId),
}
