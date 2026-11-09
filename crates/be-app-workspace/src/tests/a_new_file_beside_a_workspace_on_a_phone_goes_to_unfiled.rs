use be_sdk_beui::{BarAction, BlockLocation, ChildContent};

use super::*;

#[test]
fn a_new_file_beside_a_workspace_on_a_phone_goes_to_unfiled() {
    let (mut fixture, notes) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    fixture.test.catalog(catalog());
    let mut info = be_sdk_beui::BlockInfo::new(notes, SHOWN_TYPE, BlockParent::Root);
    info.set_name("Notes");
    fixture.test.store().add_block(info);
    show(&mut fixture, notes, None);
    let child = fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content.block_id() == Some(notes.into_bytes()))
        .expect("the block is on show")
        .child;
    fixture.test.child_bar(child, BarAction::Details);
    fixture.settle();
    fixture.test.settle();
    fixture.test.click("workspace.details.new");
    fixture.settle();

    let picker = fixture
        .test
        .placed_picker()
        .expect("New file here opens the picker");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds only a picker");
    };
    assert_eq!(
        filter.place,
        Some(BlockLocation::Unfiled),
        "nothing beside the block takes a new file, so the person chose no place for it"
    );
}
