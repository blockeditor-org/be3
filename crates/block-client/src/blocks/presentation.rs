use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Presentation {
    references: Vec<Uuid>,
}

impl Presentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PresentationOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for Presentation {
    type Operation = PresentationOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7072_6573_656e_7461_7469_6f6e_0001);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            PresentationOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(PresentationOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
