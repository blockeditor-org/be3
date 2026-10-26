use block_editor_beui::be_block::PREFERRED_EDITOR;
use block_editor_beui::{BlockInfo, ChildContent};

use super::*;

const BUILT_IN: Uuid = Uuid::from_u128(1);
const BOARD: Uuid = Uuid::from_u128(2);

#[test]
fn a_block_opens_in_the_editor_it_keeps_for_itself() {
    let (mut fixture, opened) = editor();
    let mut catalog = catalog();
    let edits = |editor: Uuid, name: &str| block_editor_beui::EditorDescriptor {
        editor: editor.into_bytes(),
        edits: block_editor_beui::Edits::Type(SHOWN_TYPE.into_bytes()),
        name: name.to_owned(),
        icon_codepoint: String::new(),
        extensions: Vec::new(),
    };
    catalog.editors = vec![edits(BUILT_IN, "List"), edits(BOARD, "Board")];
    fixture.test.catalog(catalog);
    let mut kept = BlockInfo::new(opened, SHOWN_TYPE, BlockParent::Root);
    kept.metadata.insert(&PREFERRED_EDITOR, &BOARD);
    fixture.test.store().add_block(kept);
    fixture.settle();

    show(&mut fixture, opened, None);
    let shown: Vec<Option<Uuid>> = fixture
        .test
        .children()
        .iter()
        .filter_map(|placement| match placement.content {
            ChildContent::Block { editor, .. } => Some(editor.map(Uuid::from_bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(
        shown,
        vec![Some(BOARD)],
        "the editor a block keeps for itself wins over its type's default"
    );
}
