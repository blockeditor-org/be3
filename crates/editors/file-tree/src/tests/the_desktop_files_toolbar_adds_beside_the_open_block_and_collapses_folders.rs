use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, ChildContent, FocusedBlock};

use super::*;

#[test]
fn the_desktop_files_toolbar_adds_beside_the_open_block_and_collapses_folders() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let (folder, notes, text) = (Uuid::from_u128(5), Uuid::from_u128(6), Uuid::from_u128(7));
    let mut plans = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    plans.set_name("Plans");
    plans.references = vec![notes];
    my_files(&mut fixture, vec![plans]);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .accessible_label("file-tree.new-block")
            .as_deref(),
        Some("New in My files"),
        "with nothing open, New adds to My files"
    );

    fixture.test.click(format!("file-tree.{folder}.chevron"));
    fixture.settle();
    let mut note = BlockInfo::new(notes, text, BlockParent::Block(folder));
    note.set_name("Notes");
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![note]);
    fixture.host.report_focus(FocusedBlock {
        block_id: Some(notes),
        block_type: text,
        via: vec![folder, MY_FILES],
    });
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .accessible_label("file-tree.new-block")
            .as_deref(),
        Some("New in Plans"),
        "New adds beside the open block, in the folder that holds it"
    );
    fixture
        .test
        .snapshot("the_desktop_files_toolbar_adds_beside_the_open_block");

    fixture.test.click("file-tree.new-block");
    fixture.settle();
    let picker = fixture.test.placed_picker().expect("New opens a picker");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds a picker")
    };
    assert_eq!(filter.place, Some(BlockParent::Block(folder).encode()));
    fixture.test.answer_picker(Ok(None));
    fixture.settle();

    assert!(fixture.test.shown(&format!("file-tree.{notes}.row")));
    fixture.test.click("file-tree.collapse-all");
    fixture.settle();
    assert!(
        !fixture.test.shown(&format!("file-tree.{notes}.row")),
        "Collapse all closes every open folder"
    );
    assert!(
        fixture.test.shown(&format!("file-tree.{folder}.row")),
        "and keeps the workspaces open"
    );
}
