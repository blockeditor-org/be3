use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::blob::{Blob, BlobKind, BlobOp};
use be_block::plain::{extension, media_type_of};
use be_block::{BlockMetadata, THUMBHASH, Thumbhash};
use block_type_guest::BlobFile;

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

    fn describe(header: &ImageHeader, metadata: &mut BlockMetadata) {
        if let Some(((width, height), hash)) = header.size().zip(header.thumbhash.as_ref()) {
            metadata.insert(
                &THUMBHASH,
                &Thumbhash {
                    hash: hash.clone(),
                    width,
                    height,
                },
            );
        }
    }
}

pub type ImageContent = Blob<ImageFile>;

pub type ImageOp = BlobOp<ImageHeader>;

impl ImageFile {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &[
        "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
    ];
    pub const MIME_TYPES: &'static [&'static str] = &["image/*"];

    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> ImageContent {
        ImageContent::new(
            ImageHeader {
                source_name: source_name.into(),
                ..ImageHeader::default()
            },
            data,
        )
    }
}

impl BlobFile for ImageFile {
    fn header(name: &str, _: &[u8]) -> ImageHeader {
        ImageHeader {
            source_name: name.to_owned(),
            media_type: media_type_of(name).to_owned(),
            ..ImageHeader::default()
        }
    }

    fn media_type(header: &ImageHeader) -> String {
        header.media_type.clone()
    }

    fn claims(name: &str, _: &[u8]) -> bool {
        Self::FILE_EXTENSIONS.contains(&extension(name).as_str())
    }
}
