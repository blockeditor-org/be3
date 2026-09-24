use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use be_block::pixel_art::{
    DEFAULT_PIXEL_ART_SIZE, MAX_PIXEL_ART_PALETTE_COLORS, MAX_PIXEL_ART_SIZE, PixelArtAnchor,
    PixelArtOperation, PixelColor, PixelUpdate,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PixelArt {
    references: Vec<Uuid>,
}

impl PixelArt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PixelArtBlockOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for PixelArt {
    type Operation = PixelArtBlockOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7069_7865_6c2d_6172_742d_626c_6f63_6b01);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            PixelArtBlockOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(PixelArtBlockOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
