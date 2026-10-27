use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockLocation};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn the_picker_never_offers_a_deleted_block_to_link() {
    let (mut fixture, _) = editor();
    let (kept, folder, inside) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let store = fixture.test.store();
    let mut notes = BlockInfo::new(kept, SHOWN_TYPE, BlockParent::Root);
    notes.set_name("Notes");
    store.add_block(notes);
    let mut deleted = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Detached);
    deleted.set_name("Old notes");
    store.add_block(deleted);
    let mut draft = BlockInfo::new(inside, SHOWN_TYPE, BlockParent::Block(folder));
    draft.set_name("Draft");
    store.add_block(draft);

    fixture
        .test
        .request_pick(7, filter(), BlockLocation::Root, None);
    fixture.settle();
    fixture.test.click("picker.link-existing");

    assert!(fixture.test.shown(&format!("picker.link.{kept}.row")));
    assert!(
        !fixture.test.shown(&format!("picker.link.{inside}.row")),
        "a block inside a deleted folder is deleted too, and is not offered"
    );
}
