use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Image {}

impl Image {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &[
        "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
    ];
    pub const MIME_TYPES: &'static [&'static str] = &["image/*"];

    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ImageOperation {}

impl Block for Image {
    type Operation = ImageOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696d_6167_652d_626c_6f63_6b2d_7479_7001);

    fn apply_operation(_image: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
