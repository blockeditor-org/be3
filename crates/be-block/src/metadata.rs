use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockMetadata {
    pub name: Option<String>,
    pub named_by_hand: bool,
    pub artifact: Option<ArtifactSource>,
    pub local_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactSource {
    pub source_type: Uuid,
    pub data: Vec<u8>,
}

impl BlockMetadata {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Self {
        postcard::from_bytes(bytes).unwrap_or_default()
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            named_by_hand: true,
            artifact: None,
            local_id: None,
        }
    }
}
