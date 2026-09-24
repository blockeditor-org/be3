use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod presence;

pub use be_block::canvas::{
    CanvasColor, CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle,
    CanvasLayerMove, CanvasPoint, CanvasPreviewRegion, CanvasTextAlign, CanvasTextStyle,
    CanvasTextWeight, CanvasTransform, InfiniteCanvasOperation,
};
pub use presence::CanvasCursor;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct InfiniteCanvas {
    references: Vec<Uuid>,
}

impl InfiniteCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum InfiniteCanvasBlockOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for InfiniteCanvas {
    type Operation = InfiniteCanvasBlockOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696e_6669_6e69_7465_2d63_616e_7661_7301);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            InfiniteCanvasBlockOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(InfiniteCanvasBlockOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
