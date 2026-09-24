use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use be_block::map::{
    MAX_LATITUDE, MIN_REGION_SPAN, MapColor, MapCoordinate, MapPoint, MapRegion,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Map {
    references: Vec<Uuid>,
}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MapOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for Map {
    type Operation = MapOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6d61_7076_6965_7762_6c6f_636b_0000_0001);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            MapOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(MapOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
