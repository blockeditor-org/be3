use super::*;

use block_editor_beui::be_block::BlockContent;
use text_block_types::{TextBlock, TextContent};

#[test]
fn edits_from_elsewhere_land_in_order() {
    let block = Uuid::from_u128(0x7e47);
    let store = crate::ContentStore::new(Uuid::nil());
    store.own(block, TextContent::CONTENT_TYPE);
    store.hold(None, TextBlock::of("hello world"));
    for (at, typed) in [(0, ">> "), (14, "!")] {
        let edit = TextBlock::insert(&store.content::<TextContent>(None), 2, at, typed.as_bytes())
            .expect("there is somewhere to type");
        store.edit::<TextContent>(None, &edit);
    }
    assert_eq!(
        TextBlock::text(&store.content::<TextContent>(None)),
        ">> hello world!"
    );
}
