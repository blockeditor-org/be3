use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use be_block::database_schema::{
    DatabaseBlockOptions, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseNumberOptions, DatabaseNumberScale,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatabaseSchema {
    references: Vec<Uuid>,
}

impl DatabaseSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseSchemaOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for DatabaseSchema {
    type Operation = DatabaseSchemaOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6461_7461_6261_7365_2d73_6368_656d_6101);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            DatabaseSchemaOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(DatabaseSchemaOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
