use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR};
use block_editor_beui::beui::icons::ICON_FILE_TEXT;
use block_editor_beui::{BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildOperations};

use super::*;

#[test]
fn a_block_can_override_and_reset_its_icon_and_color() {
    let mut fixture = editor();
    let kind = Uuid::from_u128(1);
    let block = Uuid::from_u128(5);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: kind.into_bytes(),
            display_name: "Note".to_owned(),
            color: Some([65, 145, 210]),
            icon_codepoint: ICON_FILE_TEXT.to_owned(),
            children: ChildOperations::default(),
        }],
        ..Catalog::default()
    });
    let mut listed = BlockInfo::new(block, kind, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    fixture.test.store().add_block(files_workspace(
        &fixture.host,
        std::slice::from_ref(&listed),
    ));
    fixture.test.store().add_block(listed);
    fixture.settle();
    fixture.test.snapshot("block_icons_inherit_the_type_color");

    fixture.choose(block, "Appearance");
    fixture.click_item("Icon");
    fixture.test.snapshot("a_block_offers_font_icons");
    fixture.click_item("Star");
    assert_eq!(
        fixture
            .test
            .store()
            .block(block)
            .unwrap()
            .metadata
            .get(&ICON)
            .as_deref(),
        Some("star")
    );

    fixture.choose(block, "Appearance");
    fixture.click_item("Icon color");
    fixture.test.snapshot("a_block_offers_icon_colors");
    fixture.click_item("Pink");
    let saved = fixture.test.store().block(block).unwrap().metadata;
    assert_eq!(saved.get(&ICON_COLOR), Some([203, 91, 125]));
    assert_eq!(saved.get(&ICON).as_deref(), Some("star"));
    fixture.test.snapshot("a_block_has_a_custom_icon_and_color");

    fixture.choose(block, "Appearance");
    fixture.click_item("Icon");
    fixture.click_item("Use block type icon");
    let saved = fixture.test.store().block(block).unwrap().metadata;
    assert!(!saved.contains(&ICON));
    assert!(saved.contains(&ICON_COLOR));

    fixture.choose(block, "Appearance");
    fixture.click_item("Icon color");
    fixture.click_item("Use block type color");
    assert!(
        !fixture
            .test
            .store()
            .block(block)
            .unwrap()
            .metadata
            .contains(&ICON_COLOR)
    );
    fixture
        .test
        .snapshot("block_icons_return_to_the_type_defaults");
}
