use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR};
use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::ICON_FILE_TEXT;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildContent, ChildOperations,
};

use super::*;

#[test]
fn icon_and_color_asks_the_host_from_the_row_and_the_row_shows_what_is_chosen() {
    let mut fixture = editor();
    let kind = Uuid::from_u128(1);
    let block = Uuid::from_u128(5);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: kind.into_bytes(),
            display_name: "Note".to_owned(),
            color: Some([65, 145, 210]),
            icon_codepoint: ICON_FILE_TEXT.as_str().to_owned(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        ..Catalog::default()
    });
    let mut listed = BlockInfo::new(block, kind, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    fixture.test.store().add_block(files_workspace(
        &fixture.host,
        std::slice::from_ref(&listed),
    ));
    fixture.test.store().add_block(listed.clone());
    fixture.settle();
    fixture.test.snapshot("block_icons_inherit_the_type_color");

    fixture.choose(block, "Icon and color…");
    fixture.settle();
    let form = fixture
        .test
        .placed(|content| *content == ChildContent::Rename(block.into_bytes()))
        .expect("choosing Icon and color places the form renaming the row's block");
    assert!(form.rect.width() > 0.0, "in a card hung from the row");
    fixture
        .test
        .snapshot("icon_and_color_hangs_a_rename_card_from_its_row");
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    listed.metadata.insert(&ICON, &"star".to_owned());
    listed.metadata.insert(&ICON_COLOR, &[203, 91, 125]);
    fixture.test.store().add_block(listed);
    fixture.settle();
    fixture.test.snapshot("a_block_has_a_custom_icon_and_color");
}
