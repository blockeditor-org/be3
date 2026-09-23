use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use be_block::database_view::{DatabaseViewKind, DatabaseViewSort, SortDirection};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatabaseView {
    references: Vec<Uuid>,
}

impl DatabaseView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseViewOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for DatabaseView {
    type Operation = DatabaseViewOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x0000_6461_7461_6261_7365_2d76_6965_7701);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            DatabaseViewOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(DatabaseViewOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
