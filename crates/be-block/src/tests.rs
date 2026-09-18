use super::*;
use crate::image::{ImageContent, ImageHeader};

mod an_image_merges_only_when_one_side_changed_it;
mod streamed_content_separates_its_header_from_its_payload;

fn header(name: &str) -> ImageHeader {
    ImageHeader {
        source_name: name.into(),
        media_type: "image/png".into(),
        width: 640,
        height: 480,
    }
}

fn image(name: &str, payload: &[u8]) -> ImageContent {
    ImageContent::new(header(name), payload.to_vec())
}
