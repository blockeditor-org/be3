use block_editor_beui::be_block::{BlockContent, WorkspaceContent};
use block_editor_beui::{BlockCommand, BlockParent};

use super::*;

#[test]
fn a_new_workspace_is_made_at_the_top_and_named_by_hand() {
    let mut fixture = editor();
    my_files(&mut fixture, Vec::new());
    fixture.settle();

    fixture.test.click("file-tree.new-workspace");
    fixture.settle();
    let store = fixture.test.store();
    let created: Vec<Uuid> = store
        .created()
        .into_iter()
        .filter(|id| {
            store.block(*id).is_some_and(|info| {
                info.block_type == WorkspaceContent::CONTENT_TYPE
                    && info.parent == BlockParent::Root
            })
        })
        .collect();
    assert_eq!(created.len(), 1, "one workspace is made at the top");
    assert!(
        fixture
            .test
            .take_block_commands()
            .contains(&(created[0], BlockCommand::Rename)),
        "the new workspace is named in the rename dialog"
    );
}
