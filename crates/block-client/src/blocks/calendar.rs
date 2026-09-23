use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Calendar {}

impl Calendar {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CalendarOperation {}

impl Block for Calendar {
    type Operation = CalendarOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6361_6c65_6e64_6172_2d62_6c6f_636b_0001);

    fn apply_operation(_calendar: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
