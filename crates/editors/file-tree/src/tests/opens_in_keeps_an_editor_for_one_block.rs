use block_editor_beui::be_block::be_model::from_cbor;
use block_editor_beui::be_block::PREFERRED_EDITOR;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildOperations, EditorDescriptor,
};

use super::*;

#[test]
fn opens_in_keeps_an_editor_for_one_block() {
    let mut fixture = editor();
    let block_type = Uuid::from_u128(1);
    let block = Uuid::from_u128(2);
    let (board, list) = (Uuid::from_u128(3), Uuid::from_u128(4));
    let edits = |editor: Uuid, name: &str| EditorDescriptor {
        editor: editor.into_bytes(),
        edits: block_editor_beui::Edits::Type(block_type.into_bytes()),
        name: name.to_owned(),
        icon_codepoint: String::new(),
    };
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: "Tasks".to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        templates: Vec::new(),
        editors: vec![edits(list, "List"), edits(board, "Board")],
    });
    let mut listed = BlockInfo::new(block, block_type, BlockParent::Block(MY_FILES));
    listed.set_name("Launch");
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture.open_menu(block);
    fixture.click_item("Opens in");
    assert!(fixture.offers("Your default editor"));
    fixture
        .test
        .snapshot("opens_in_keeps_an_editor_for_one_block");
    fixture.click_item("Board");
    let chosen: Vec<(Uuid, Option<Uuid>)> = fixture
        .test
        .take_definitions(PREFERRED_EDITOR.id)
        .into_iter()
        .map(|(block, value)| (block, value.and_then(|value| from_cbor(&value))))
        .collect();
    assert_eq!(chosen, vec![(block, Some(board))]);
}
