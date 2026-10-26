use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use workspace_ui_types::EditorView;

use super::*;

#[test]
fn a_folder_reopened_from_the_view_is_listed_before_the_files_are_shown() {
    let folder = Uuid::new_v4();
    let child = Uuid::new_v4();
    let mut fixture = viewed(EditorView::document(None));
    let mut listed = BlockInfo::new(folder, Uuid::nil(), BlockParent::Block(MY_FILES));
    listed.references = vec![child];
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    fixture.test.click(format!("file-tree.{folder}.chevron"));
    fixture.settle();
    let saved = fixture.test.content::<EditorViewContent>(None);

    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_view_block(Some(tree));
    let files_host = host.clone();
    let mut reopened = BeuiTest::<FileTreeApp>::new(OwnedEditor::new(host, tree));
    let mut held = BlockInfo::new(folder, Uuid::nil(), BlockParent::Block(MY_FILES));
    held.references = vec![child];
    reopened
        .store()
        .add_block(files_workspace(&files_host, std::slice::from_ref(&held)));
    reopened.store().add_block(held);
    reopened.store().add_block(BlockInfo::new(
        child,
        Uuid::nil(),
        BlockParent::Block(folder),
    ));
    reopened.hold(None, saved);

    let folder_row = format!("file-tree.{folder}.row");
    let child_row = format!("file-tree.{child}.row");
    for _ in 0..20 {
        reopened.step(Vec::new());
        assert!(
            !reopened.shown(&folder_row) || reopened.shown(&child_row),
            "the files are never shown with the folder left open still unlisted"
        );
    }
    assert!(reopened.shown(&child_row));
}
