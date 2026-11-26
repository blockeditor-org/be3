use be_sdk_beui::{BlockInfo, BlockLocation};

use super::*;

/// The picker's rows carry what the account has not read in each block (`BlockInfo::unread`)
/// as a badge, with the block's name bold, as the files do.
#[test]
fn the_pickers_rows_show_what_is_unread_in_each_block() {
    let (mut fixture, _) = editor();
    let space = Uuid::new_v4();
    let lunch = unread_chat(&mut fixture, BlockParent::Block(space), "Lunch", 2);
    let notes = Uuid::new_v4();
    let mut workspace = BlockInfo::new(space, Uuid::new_v4(), BlockParent::Root);
    workspace.set_name("Team space");
    workspace.references = vec![lunch, notes];
    fixture.test.store().add_block(workspace);
    let mut read = BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Block(space));
    read.set_intrinsic_name("Notes".to_owned());
    fixture.test.store().add_block(read);

    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.settle();
    fixture.test.click("picker.link-existing");
    fixture.settle();

    assert!(
        fixture.test.shown(&format!("picker.{lunch}.unread")),
        "the unread block's row carries a badge"
    );
    assert!(
        !fixture.test.shown(&format!("picker.{notes}.unread")),
        "a block with nothing unread carries none"
    );
    fixture
        .test
        .snapshot("the_pickers_rows_show_what_is_unread_in_each_block");
}
