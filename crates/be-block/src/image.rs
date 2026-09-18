use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, Merge,
    streamed::{Streamed, decode_streamed, encode_streamed},
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageHeader {
    pub source_name: String,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

impl ImageHeader {
    pub fn size(&self) -> Option<(u32, u32)> {
        (self.width > 0 && self.height > 0).then_some((self.width, self.height))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImageContent {
    header: ImageHeader,
    data: Vec<u8>,
}

impl ImageContent {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &[
        "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
    ];

    pub fn new(header: ImageHeader, data: Vec<u8>) -> Self {
        Self { header, data }
    }

    pub const fn header(&self) -> &ImageHeader {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn set_header(&mut self, header: ImageHeader) {
        self.header = header;
    }
}

impl BlockContent for ImageContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696d_6167_652d_626c_6f63_6b2d_7479_7002);

    fn encode(&self) -> Vec<u8> {
        encode_streamed(&self.header, &self.data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let (header, data) = decode_streamed(bytes)?;
        Ok(Self { header, data })
    }

    fn name(&self) -> Option<String> {
        let name = self.header.source_name.trim();
        (!name.is_empty()).then(|| name.to_owned())
    }
}

impl Streamed for ImageContent {
    type Header = ImageHeader;

    fn header(&self) -> Self::Header {
        self.header.clone()
    }

    fn payload(&self) -> &[u8] {
        &self.data
    }

    fn from_parts(header: Self::Header, payload: Vec<u8>) -> Self {
        Self {
            header,
            data: payload,
        }
    }
}

impl Merge for ImageContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        if ours == theirs || theirs == base {
            return MergeResult::Clean(ours.clone());
        }
        if ours == base {
            return MergeResult::Clean(theirs.clone());
        }
        MergeResult::Conflicted {
            value: ours.clone(),
            conflicts: 1,
        }
    }
}
