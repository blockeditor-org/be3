use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Counter {}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CounterOperation {}

impl Block for Counter {
    type Operation = CounterOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_0001);

    fn apply_operation(_counter: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
