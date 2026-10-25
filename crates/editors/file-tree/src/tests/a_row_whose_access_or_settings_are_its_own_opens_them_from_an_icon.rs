use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{Root, SETTINGS, settings::Settings};
use block_editor_beui::{AccessLevel, BlockCommand, BlockInfo, BlockParent};

use super::*;

#[test]
fn a_row_whose_access_or_settings_are_its_own_opens_them_from_an_icon() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    let (plans, notes, viewed) = (Uuid::from_u128(3), Uuid::from_u128(4), Uuid::from_u128(5));
    let settings = Uuid::from_u128(6);
    let mut shared = BlockInfo::new(plans, text_type, BlockParent::Block(MY_FILES));
    shared.set_intrinsic_name("Plans");
    shared.sets_access = true;
    shared.metadata.insert(&SETTINGS, &BlockRef(settings));
    let mut configured = BlockInfo::new(notes, text_type, BlockParent::Block(MY_FILES));
    configured.set_intrinsic_name("Notes");
    configured.metadata.insert(&SETTINGS, &BlockRef(settings));
    let mut only_viewed = BlockInfo::new(viewed, text_type, BlockParent::Block(MY_FILES));
    only_viewed.set_intrinsic_name("Shared with me to read");
    only_viewed.sets_access = true;
    only_viewed.access = AccessLevel::View;
    my_files(&mut fixture, vec![shared, configured, only_viewed]);
    fixture.settle();

    fixture
        .test
        .snapshot("a_row_whose_access_or_settings_are_its_own_opens_them_from_an_icon");
    assert!(
        fixture
            .test
            .document()
            .find_test_id(&format!("file-tree.{viewed}.share"))
            .is_none(),
        "only someone who manages a block is offered its sharing"
    );
    assert!(
        fixture
            .test
            .document()
            .find_test_id(&format!("file-tree.{notes}.share"))
            .is_none()
    );

    fixture.test.take_block_commands();
    fixture
        .test
        .click(format!("file-tree.{plans}.share").as_str());
    fixture.settle();
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(plans, BlockCommand::Share)],
        "the share icon opens the share dialog"
    );
    assert!(
        fixture.opened().is_empty(),
        "the icon does not also open the row"
    );

    fixture
        .test
        .click(format!("file-tree.{notes}.settings").as_str());
    fixture.settle();
    let opens = fixture.test.take_opens();
    assert_eq!(
        opens
            .iter()
            .map(|open| (open.block_id, open.block_type))
            .collect::<Vec<_>>(),
        vec![(settings, <Settings as Root>::CONTENT_TYPE)],
        "the settings icon opens the block's settings"
    );
}
