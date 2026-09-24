use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use be_block::video::{
    DEFAULT_CLIP_SECONDS, MAX_CLIP_LENGTH, VideoAttachment, VideoClip, VideoClipTiming, VideoEffect,
    VideoFrameRate,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Video {
    references: Vec<Uuid>,
}

impl Video {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_references(references: Vec<Uuid>) -> Self {
        Self { references }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum VideoOperation {
    SetReferences { references: Vec<Uuid> },
}

impl Block for Video {
    type Operation = VideoOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7669_6465_6f5f_626c_6f63_6b00_0000_0001);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            VideoOperation::SetReferences { references } => {
                block.references.clone_from(references);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }

    fn bridged_references(references: Vec<Uuid>) -> Option<Self::Operation> {
        Some(VideoOperation::SetReferences { references })
    }
}

#[cfg(test)]
mod tests;
