use block_editor_beui::BlockParent;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Key;
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn a_new_workspace_is_made_at_the_top_and_named_in_place() {
    let mut fixture = editor();
    my_files(&mut fixture, Vec::new());
    fixture.settle();

    fixture.test.click("file-tree.more");
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
                    && info.parent == Some(BlockParent::Root)
            })
        })
        .collect();
    assert_eq!(created.len(), 1, "one workspace is made at the top");
    let made = created[0];
    let field = format!("file-tree.{made}.rename");
    assert!(
        fixture.test.shown(&field),
        "the new workspace's name is edited in place once its row is shown"
    );
    fixture.test.type_text("Team");
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_renames(),
        vec![(made, Some("Team".to_owned()))],
        "Enter names the workspace"
    );
    assert!(
        fixture
            .test
            .placed(|content| matches!(content, block_editor_beui::ChildContent::Rename(_)))
            .is_none(),
        "no rename form is placed"
    );
}
