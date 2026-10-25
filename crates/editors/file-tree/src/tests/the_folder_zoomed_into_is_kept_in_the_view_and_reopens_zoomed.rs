use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::EditorView;
use workspace_ui_types::FolderContent;

use super::*;

fn with_folder(fixture: &mut Fixture, folder: Uuid, notes: Uuid) {
    folders_zoom(fixture);
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(7), BlockParent::Block(MY_FILES));
    listed.set_intrinsic_name("Notes".to_owned());
    my_files(fixture, vec![projects, listed]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), Vec::new());
    fixture.settle();
}

#[test]
fn the_folder_zoomed_into_is_kept_in_the_view_and_reopens_zoomed() {
    let (folder, notes) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let mut fixture = viewed_on_phone(EditorView::document(None));
    with_folder(&mut fixture, folder, notes);
    fixture.test.click(format!("file-tree.{folder}.row"));
    fixture.settle();
    assert!(fixture.test.shown("file-tree.back"));
    let saved = fixture.test.content::<EditorViewContent>(None);

    let mut reopened = viewed_on_phone(saved);
    with_folder(&mut reopened, folder, notes);
    assert!(
        reopened.test.shown("file-tree.back"),
        "the folder is zoomed into again without a tap, though its row arrived after the view"
    );
    assert!(!reopened.test.shown(&format!("file-tree.{notes}.row")));

    reopened.test.click("file-tree.back");
    reopened.settle();
    let left = reopened.test.content::<EditorViewContent>(None);
    let mut again = viewed_on_phone(left);
    with_folder(&mut again, folder, notes);
    assert!(
        again.test.shown(&format!("file-tree.{notes}.row")),
        "going up to My files is kept too"
    );
    assert!(again.test.shown("file-tree.back"));
    assert!(!again.test.shown("file-tree.shared.row"));
}
