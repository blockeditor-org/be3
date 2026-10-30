use block_editor_beui::DialogAnswer;
use block_editor_beui::be_block::metadata::{BlockMetadata, ICON, ICON_COLOR};
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

fn saved(fixture: &mut Fixture<RenameApp>, block: Uuid) -> BlockMetadata {
    fixture.test.store().block(block).unwrap().metadata
}

#[test]
fn the_rename_card_sets_a_blocks_name_icon_and_color() {
    let (mut workspace, notes) = editor();
    workspace.test.catalog(catalog());
    let mut info = block_editor_beui::BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Notes");
    workspace.test.store().add_block(info);
    let mut fixture = renaming(&workspace, notes);
    fixture.settle();
    assert!(
        fixture.test.shown("rename.icon.default"),
        "the card opens on the block type's icon"
    );
    fixture
        .test
        .snapshot("the_rename_card_offers_a_name_and_every_icon_and_color");

    fixture.test.click("rename.name");
    fixture.settle();
    fixture.test.key(Key::A, Modifiers::CTRL);
    fixture.test.type_text("Journal");
    fixture.test.click("rename.search");
    fixture.settle();
    fixture.test.type_text("duplicate");
    fixture.settle();
    assert!(
        fixture.test.shown("rename.icon.copy-plus"),
        "a search finds icons by their tags"
    );
    assert!(
        !fixture.test.shown("rename.icon.default"),
        "and leaves out what does not match"
    );
    fixture.test.snapshot("the_rename_card_finds_icons_by_tag");
    fixture.test.click("rename.icon.copy-plus");
    fixture.settle();
    assert_eq!(
        saved(&mut fixture, notes).get(&ICON).as_deref(),
        Some("copy-plus")
    );

    fixture.test.click("rename.color.pink");
    fixture.settle();
    assert_eq!(
        saved(&mut fixture, notes).get(&ICON_COLOR),
        Some([203, 91, 125])
    );
    assert!(
        fixture.test.shown("rename.search"),
        "the card stays open to keep choosing"
    );

    fixture.test.click("rename.color.default");
    fixture.settle();
    let metadata = saved(&mut fixture, notes);
    assert!(!metadata.contains(&ICON_COLOR), "the type's color is back");
    assert_eq!(metadata.get(&ICON).as_deref(), Some("copy-plus"));

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_dialog_closes(),
        [DialogAnswer::Dismissed],
        "Escape closes the card"
    );
    assert_eq!(
        fixture.test.take_renames().last(),
        Some(&(notes, Some("Journal".to_owned()))),
        "and keeps the name typed into it"
    );
}
