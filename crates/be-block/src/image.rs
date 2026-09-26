use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DerivedMetadata;
use crate::blob::{Blob, BlobKind, BlobOp};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageHeader {
    pub source_name: String,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    pub failure: Option<String>,
    pub thumbhash: Option<Vec<u8>>,
}

impl ImageHeader {
    pub fn size(&self) -> Option<(u32, u32)> {
        (self.width > 0 && self.height > 0).then_some((self.width, self.height))
    }
}

pub struct ImageFile;

impl BlobKind for ImageFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696d_6167_652d_626c_6f63_6b2d_7479_7001);

    type Header = ImageHeader;

    fn name(header: &ImageHeader) -> &str {
        &header.source_name
    }

    fn derived_metadata(header: &ImageHeader) -> DerivedMetadata {
        DerivedMetadata {
            thumbhash: header.thumbhash.clone(),
        }
    }
}

pub type ImageContent = Blob<ImageFile>;

pub type ImageOp = BlobOp<ImageHeader>;

impl Blob<ImageFile> {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &[
        "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
    ];
    pub const MIME_TYPES: &'static [&'static str] = &["image/*"];

    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> Self {
        Self::new(
            ImageHeader {
                source_name: source_name.into(),
                ..ImageHeader::default()
            },
            data,
        )
    }
}
