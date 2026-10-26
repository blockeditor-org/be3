use super::*;
use workspace_ui_types::FolderContent;

use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockCommand, BlockInfo};

#[test]
fn a_deleted_block_says_so_above_it_and_can_be_restored() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 600.0)));
    let folder = Uuid::new_v4();
    let store = fixture.test.store();
    let mut deleted = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Detached);
    deleted.set_name("Old notes");
    deleted.references = vec![opened];
    store.add_block(deleted);
    let mut inside = BlockInfo::new(opened, SHOWN_TYPE, BlockParent::Block(folder));
    inside.set_name("Plans");
    store.add_block(inside);

    show(&mut fixture, opened, None);

    assert!(
        fixture.says("This block is inside “Old notes”, which is in Recently Deleted."),
        "the bar names the deleted folder it is inside"
    );
    fixture
        .test
        .snapshot("a_deleted_block_says_so_above_it_and_can_be_restored");

    fixture.test.take_block_commands();
    fixture.click_text("Restore");
    assert!(
        fixture
            .test
            .take_block_commands()
            .contains(&(folder, BlockCommand::Restore)),
        "restoring brings back the deleted folder, and the block with it"
    );
}
