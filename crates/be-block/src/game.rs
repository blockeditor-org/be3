use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blob::{Blob, BlobKind};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameModuleHeader {
    pub source_name: String,
}

pub struct GameModuleFile;

impl BlobKind for GameModuleFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6761_6d65_2d6d_6f64_756c_652d_6366_0002);

    type Header = GameModuleHeader;

    fn name(header: &GameModuleHeader) -> &str {
        &header.source_name
    }
}

pub type GameModuleContent = Blob<GameModuleFile>;

impl Blob<GameModuleFile> {
    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> Self {
        Self::new(
            GameModuleHeader {
                source_name: source_name.into(),
            },
            data,
        )
    }
}
