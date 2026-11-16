use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::be_block_content::EditorView;
use be_sdk_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;

use super::*;

/// A tree viewed through a block whose content this build cannot read (written by a newer one,
/// or damaged) reads that view as failed, not loading, and shows its files with a view of its
/// own.
#[test]
fn a_tree_whose_view_block_cannot_be_read_still_shows() {
    let (tree, view) = (Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_view_block(Some(view));
    let mut test = BeuiTest::<FileTreeRoot>::new(OwnedEditor::new(host.clone(), tree));
    let mine = BlockInfo::new(
        Uuid::new_v4(),
        Uuid::from_u128(0x6e6f_7465),
        BlockParent::Block(MY_FILES),
    );
    test.store()
        .add_block(files_workspace(&host, std::slice::from_ref(&mine)));
    test.store().add_block(mine.clone());
    test.store().add_block(BlockInfo::new(
        view,
        EditorViewContent::CONTENT_TYPE,
        BlockParent::Block(tree),
    ));
    test.hold(None, EditorView::document(None));
    test.store().hold_unreadable::<EditorViewContent>(Some(view));
    test.run();
    assert!(
        test.shown(&format!("file-tree.{}.row", mine.id)),
        "the files show though the view block cannot be read"
    );
}
