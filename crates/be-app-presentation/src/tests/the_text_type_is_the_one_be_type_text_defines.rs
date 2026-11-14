use be_sdk_beui::be_block_content::BlockContent;
use be_type_text::TextContent;

use crate::root::TEXT_TYPE;

#[test]
fn the_text_type_is_the_one_be_type_text_defines() {
    assert_eq!(TEXT_TYPE, TextContent::CONTENT_TYPE);
}
