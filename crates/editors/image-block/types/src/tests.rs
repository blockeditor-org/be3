use be_block::{BlockContent, LiveEdit, Merge};
use be_commit::MergeResult;
use block_type_guest::api::{Change, FileChange, PlainFile, guest::exports::replicas::Guest};

use crate::{ImageContent, ImageFile, ImageHeader, ImageOp};

mod a_picture_file_becomes_an_image_and_back;
mod an_image_is_decoded_into_its_header_and_keeps_its_bytes;
mod an_image_merges_only_when_one_side_changed_it;

fn image(name: &str, payload: &[u8]) -> ImageContent {
    ImageContent::new(
        ImageHeader {
            source_name: name.into(),
            media_type: "image/png".into(),
            width: 640,
            height: 480,
            failure: None,
            thumbhash: None,
        },
        payload.to_vec(),
    )
}
