use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_link_to_a_block_you_cannot_see_reads_no_access_beside_one_you_can() {
    let block = BLOCK_ID;
    let notes = Uuid::from_u128(0x0a);
    let hidden = Uuid::from_u128(0x0b);
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_organization_id(ORGANIZATION_ID);
    let mut seen = BlockInfo::new(notes, TextContent::CONTENT_TYPE, BlockParent::Root);
    seen.set_name("Meeting notes");
    let mut unseen = BlockInfo::new(hidden, TextContent::CONTENT_TYPE, BlockParent::Root);
    unseen.parent = None;
    unseen.author = None;
    unseen.access = AccessLevel::KnowExists;
    host.set_blocks(BlockQuery::References(block), vec![seen, unseen]);
    let link = |target: Uuid| block_url("https://example.com/", ORGANIZATION_ID, target);
    let mut editor: BeuiTest<TextApp> = BeuiTest::new(OwnedEditor::new(host, block));
    editor.hold(
        None,
        TextBlock::of(&format!(
            "Read {} first.\n\nThen {} after it.\n",
            link(notes),
            link(hidden)
        )),
    );
    editor.run();

    editor.snapshot("a_link_to_a_block_you_cannot_see_reads_no_access_beside_one_you_can");
}
