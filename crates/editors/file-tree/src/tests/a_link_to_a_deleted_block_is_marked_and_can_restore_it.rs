use block_editor_beui::{
    BlockCommand, BlockInfo, BlockParent, BlockQuery, BlockTypeDescriptor, Catalog, ChildOperations,
};

use super::*;

#[test]
fn a_link_to_a_deleted_block_is_marked_and_can_restore_it() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: text_type.into_bytes(),
            display_name: "Text".to_owned(),
            color: None,
            icon_codepoint: block_editor_beui::beui::icons::ICON_FILE_TEXT
                .as_str()
                .to_owned(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        templates: Vec::new(),
        editors: Vec::new(),
    });
    let (notes, draft) = (Uuid::from_u128(3), Uuid::from_u128(4));
    let mut kept = BlockInfo::new(notes, text_type, BlockParent::Block(MY_FILES));
    kept.set_name("Notes");
    let mut deleted = BlockInfo::new(draft, text_type, BlockParent::Detached);
    deleted.set_name("Draft");
    deleted.deleted = true;
    let inside = vec![kept, deleted.clone()];
    let workspace = files_workspace(&fixture.host, &inside);
    fixture.host.set_blocks(BlockQuery::Roots, vec![workspace]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), inside);
    fixture.settle();
    fixture.test.take_block_commands();
    fixture.choose(draft, "Restore");
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(draft, BlockCommand::Restore)],
        "the link offers to bring the deleted block back"
    );

    fixture.test.click("file-tree.orphans.chevron");
    fixture.settle();
    fixture.host.set_blocks(BlockQuery::Detached, vec![deleted]);
    fixture.settle();

    fixture
        .test
        .snapshot("a_link_to_a_deleted_block_is_marked_and_can_restore_it");

    fixture.open_menu(notes);
    assert!(
        !fixture.offers("Restore"),
        "a block that is not deleted has nothing to restore"
    );
}
