use be_sdk_beui::be_block_content::EditorView;
use be_sdk_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;

use super::*;

/// A tree viewed through a block it cannot see (deleted elsewhere, or no longer shared), whose
/// content therefore never loads, shows its files all the same.
#[test]
fn a_tree_whose_view_block_never_loads_still_shows() {
    let (tree, gone) = (Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_view_block(Some(gone));
    let mut test = BeuiTest::<FileTreeRoot>::new(OwnedEditor::new(host.clone(), tree));
    let mine = BlockInfo::new(
        Uuid::new_v4(),
        Uuid::from_u128(0x6e6f_7465),
        BlockParent::Block(MY_FILES),
    );
    test.store()
        .add_block(files_workspace(&host, std::slice::from_ref(&mine)));
    test.store().add_block(mine.clone());
    test.hold(None, EditorView::document(None));
    for _ in 0..20 {
        test.step(Vec::new());
    }
    assert!(
        test.shown(&format!("file-tree.{}.row", mine.id)),
        "the files show though the view block never loads"
    );
}
