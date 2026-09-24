use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Audio {}

impl Audio {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum AudioOperation {}

impl Block for Audio {
    type Operation = AudioOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6175_6469_6f2d_626c_6f63_6b2d_7479_7001);

    fn apply_operation(_audio: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
