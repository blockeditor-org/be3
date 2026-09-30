use block_editor_beui::be_block::{EditorView, FILES_EDITOR};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

fn with_folder(fixture: &mut Fixture, folder: Uuid) {
    let mut listed = BlockInfo::new(folder, Uuid::nil(), BlockParent::Root);
    listed.references = vec![Uuid::new_v4()];
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();
}

fn asks_for_children(fixture: &Fixture, folder: Uuid) -> bool {
    fixture
        .host
        .watched_blocks()
        .contains(&BlockQuery::References(folder))
}

#[test]
fn an_opened_folder_is_kept_open_in_the_view_and_reopens_open() {
    let folder = Uuid::new_v4();
    let mut fixture = viewed(EditorView::document(FILES_EDITOR, None));
    with_folder(&mut fixture, folder);
    assert!(!asks_for_children(&fixture, folder));

    fixture.test.click(&format!("file-tree.{folder}.chevron"));
    fixture.settle();
    let saved = fixture.test.content::<EditorViewContent>(None);

    let mut reopened = viewed(saved);
    with_folder(&mut reopened, folder);

    assert!(
        asks_for_children(&reopened, folder),
        "the folder is open again without a click"
    );
}
