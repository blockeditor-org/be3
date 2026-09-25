use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blob::{Blob, BlobKind};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioHeader {
    pub source_name: String,
    pub media_type: String,
}

pub struct AudioFile;

impl BlobKind for AudioFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6175_6469_6f2d_626c_6f63_6b2d_7479_7001);

    type Header = AudioHeader;

    fn name(header: &AudioHeader) -> &str {
        &header.source_name
    }
}

pub type AudioContent = Blob<AudioFile>;

impl Blob<AudioFile> {
    pub fn from_file(
        source_name: impl Into<String>,
        media_type: impl Into<String>,
        data: Vec<u8>,
    ) -> Result<Self, String> {
        if data.is_empty() {
            return Err("audio data must not be empty".into());
        }
        Ok(Self::new(
            AudioHeader {
                source_name: source_name.into(),
                media_type: media_type.into(),
            },
            data,
        ))
    }
}
