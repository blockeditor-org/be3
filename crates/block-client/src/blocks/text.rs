use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextDocument {
    references: Vec<Uuid>,
}

impl TextDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum TextDocumentOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for TextDocument {
    type Operation = TextDocumentOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            TextDocumentOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(TextDocumentOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
