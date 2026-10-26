use block_editor_beui::be_block::be_model::{BlockRef, from_cbor};
use block_editor_beui::be_block::{
    BlockContent, TAG_COLOR, TAG_DEFINITIONS, TAGGED, TagColor, Tags, TagsContent,
};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_tag_from_the_workspace_marks_a_block_and_shows_on_its_row() {
    let mut fixture = editor();
    let (notes, plans, tags, urgent, later) = (
        Uuid::from_u128(5),
        Uuid::from_u128(6),
        Uuid::from_u128(7),
        Uuid::from_u128(8),
        Uuid::from_u128(9),
    );
    let tag = |id: Uuid, name: &str, color: TagColor| {
        let mut info = BlockInfo::new(id, TagsContent::CONTENT_TYPE, BlockParent::Block(tags));
        info.block_type = block_editor_beui::be_block::TagContent::CONTENT_TYPE;
        info.set_name(name);
        info.metadata.derived.insert(
            TAG_COLOR.id,
            block_editor_beui::be_block::be_model::to_cbor(&color),
        );
        fixture.host.set_blocks(BlockQuery::Block(id), vec![info]);
    };
    fixture.host.set_blocks(
        BlockQuery::Block(tags),
        vec![BlockInfo::new(
            tags,
            TagsContent::CONTENT_TYPE,
            BlockParent::Block(MY_FILES),
        )],
    );
    tag(urgent, "Urgent", TagColor::Red);
    tag(later, "Later", TagColor::Blue);
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    let mut other = BlockInfo::new(plans, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    other.set_name("Plans");
    other.metadata.insert(&TAGGED, &vec![BlockRef(later)]);
    let mut files = files_workspace(&fixture.host, &[listed.clone(), other.clone()]);
    files.metadata.insert(&TAG_DEFINITIONS, &BlockRef(tags));
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![files.clone()]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), vec![listed, other]);
    for block in [notes, plans] {
        fixture
            .host
            .set_blocks(BlockQuery::Parents(block), vec![files.clone()]);
    }
    let mut defined = TagsContent::default();
    defined.apply(&Tags::add(urgent));
    defined.apply(&Tags::add(later));
    fixture.test.hold(Some(tags), defined);
    fixture.settle();

    fixture.open_menu(notes);
    fixture.click_item("Tags");
    assert!(
        fixture.offers("Urgent"),
        "the menu offers the workspace's tags"
    );
    fixture.click_item("Urgent");
    let tagged: Vec<(Uuid, Vec<BlockRef>)> = fixture
        .test
        .take_definitions(TAGGED.id)
        .into_iter()
        .map(|(block, value)| {
            (
                block,
                value
                    .and_then(|value| from_cbor(&value))
                    .unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(tagged, vec![(notes, vec![BlockRef(urgent)])]);
    fixture
        .test
        .key(block_editor_beui::beui::Key::Escape, Modifiers::NONE);
    fixture.settle();

    assert!(fixture.says("Later"), "a row shows the tags on its block");
    fixture
        .test
        .snapshot("a_tag_from_the_workspace_marks_a_block_and_shows_on_its_row");
}
