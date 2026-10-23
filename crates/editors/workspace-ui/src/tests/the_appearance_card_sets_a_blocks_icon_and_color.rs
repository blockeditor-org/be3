use block_editor_beui::ShellDialog;
use block_editor_beui::be_block::metadata::{BlockMetadata, ICON, ICON_COLOR};
use block_editor_beui::beui::{Key, Modifiers, Pos2};

use super::*;

fn saved(fixture: &mut Fixture, block: Uuid) -> BlockMetadata {
    fixture.test.store().block(block).unwrap().metadata
}

#[test]
fn the_appearance_card_sets_a_blocks_icon_and_color() {
    let (mut fixture, notes) = editor();
    fixture.test.catalog(catalog());
    let mut info = block_editor_beui::BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Notes");
    fixture.test.store().add_block(info);
    show(&mut fixture, notes, None);

    fixture.test.show_dialog(
        notes,
        ShellDialog::Appearance {
            anchor: Some(Rect::from_min_size(
                Pos2::new(320.0, 96.0),
                Vec2::new(28.0, 28.0),
            )),
            within: Some(notes.into_bytes()),
        },
    );
    fixture.settle();
    assert!(
        fixture.test.shown("appearance.icon.default"),
        "the card opens on the block type's icon"
    );
    fixture
        .test
        .snapshot("the_appearance_card_offers_every_icon_and_color");

    fixture.test.type_text("duplicate");
    fixture.settle();
    assert!(
        fixture.test.shown("appearance.icon.copy-plus"),
        "a search finds icons by their tags"
    );
    assert!(
        !fixture.test.shown("appearance.icon.default"),
        "and leaves out what does not match"
    );
    fixture
        .test
        .snapshot("the_appearance_card_finds_icons_by_tag");
    fixture.test.click("appearance.icon.copy-plus");
    fixture.settle();
    assert_eq!(
        saved(&mut fixture, notes).get(&ICON).as_deref(),
        Some("copy-plus")
    );

    fixture.test.click("appearance.color.pink");
    fixture.settle();
    assert_eq!(
        saved(&mut fixture, notes).get(&ICON_COLOR),
        Some([203, 91, 125])
    );
    assert!(
        fixture.test.shown("appearance.search"),
        "the card stays open to keep choosing"
    );

    fixture.test.click("appearance.color.default");
    fixture.settle();
    let metadata = saved(&mut fixture, notes);
    assert!(!metadata.contains(&ICON_COLOR), "the type's color is back");
    assert_eq!(metadata.get(&ICON).as_deref(), Some("copy-plus"));

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(
        !fixture.test.shown("appearance.search"),
        "Escape closes the card"
    );
}
