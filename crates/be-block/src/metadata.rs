use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockMetadata {
    pub name: Option<String>,
    pub named_by_hand: bool,
    pub artifact: Option<ArtifactSource>,
    pub local_id: Option<Uuid>,
    pub derived: DerivedMetadata,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedMetadata {
    pub thumbhash: Option<Thumbhash>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Thumbhash {
    pub hash: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactSource {
    pub source_type: Uuid,
    pub data: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
enum StoredMetadata<'a> {
    V1(Cow<'a, BlockMetadata>),
}

impl BlockMetadata {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(&StoredMetadata::V1(Cow::Borrowed(self))).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, crate::ContentError> {
        match postcard::from_bytes(bytes)
            .map_err(|_| crate::ContentError::Malformed("block metadata"))?
        {
            StoredMetadata::V1(metadata) => Ok(metadata.into_owned()),
        }
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            named_by_hand: true,
            artifact: None,
            local_id: None,
            derived: DerivedMetadata::default(),
        }
    }
}
