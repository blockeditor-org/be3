use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LogicGame {
    references: Vec<Uuid>,
}

impl LogicGame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LogicGameOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for LogicGame {
    type Operation = LogicGameOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6c6f_6769_632d_6761_6d65_2d62_6c6b_0101);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            LogicGameOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(LogicGameOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
