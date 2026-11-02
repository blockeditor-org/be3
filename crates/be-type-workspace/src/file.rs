use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::blob::{Blob, BlobKind};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileHeader {
    pub source_name: String,
    pub media_type: String,
}

pub struct AnyFile;

impl BlobKind for AnyFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6669_6c65_2d62_6c6f_636b_2d74_7970_0001);

    type Header = FileHeader;

    fn name(header: &FileHeader) -> &str {
        &header.source_name
    }
}

pub type FileContent = Blob<AnyFile>;
