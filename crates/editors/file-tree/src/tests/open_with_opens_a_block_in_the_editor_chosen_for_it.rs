use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildOperations, EditorDescriptor,
};

use super::*;

#[test]
fn open_with_opens_a_block_in_the_editor_chosen_for_it() {
    let mut fixture = editor();
    let block_type = Uuid::from_u128(1);
    let block = Uuid::from_u128(2);
    let (built_in, debug) = (Uuid::from_u128(3), Uuid::from_u128(4));
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: "Note".to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        templates: Vec::new(),
        editors: vec![
            EditorDescriptor {
                editor: built_in.into_bytes(),
                edits: block_editor_beui::Edits::Type(block_type.into_bytes()),
                name: "Note".to_owned(),
                icon_codepoint: String::new(),
                extensions: Vec::new(),
            },
            EditorDescriptor {
                editor: debug.into_bytes(),
                edits: block_editor_beui::Edits::Any,
                name: "Debug view".to_owned(),
                icon_codepoint: String::new(),
                extensions: Vec::new(),
            },
        ],
    });
    let mut listed = BlockInfo::new(block, block_type, BlockParent::Block(MY_FILES));
    listed.set_name("Notes".to_owned());
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture.choose(block, "Open with");
    fixture
        .test
        .snapshot("open_with_opens_a_block_in_the_editor_chosen_for_it");
    let document = fixture.test.document();
    let item = document
        .root()
        .and_then(|root| open_item(&document, root, "Debug view", false))
        .expect("Open with offers every editor of the block's type");
    drop(document);
    fixture.test.click(item);
    fixture.settle();

    let opens = fixture.test.take_opens();
    assert_eq!(opens.len(), 1, "choosing an editor opens the block once");
    assert_eq!(opens[0].block_id, block);
    assert_eq!(opens[0].editor, Some(debug));
}
