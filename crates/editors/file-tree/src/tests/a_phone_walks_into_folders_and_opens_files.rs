use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn a_phone_walks_into_folders_and_opens_files() {
    let mut fixture = phone();
    containers_hold_blocks(&mut fixture);
    let text_type = Uuid::from_u128(7);
    let folder = Uuid::from_u128(2);
    let notes = Uuid::from_u128(3);
    let inside = Uuid::from_u128(4);
    let files = MY_FILES;
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(files),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    projects.references = vec![inside];
    let mut listed = BlockInfo::new(notes, text_type, BlockParent::Block(files));
    listed.set_intrinsic_name("Notes".to_owned());
    my_files(&mut fixture, vec![projects, listed]);
    fixture.settle();
    assert!(fixture.says("My files"));
    assert!(fixture.test.shown("file-tree.shared.row"));
    assert!(
        fixture.test.shown("file-tree.new"),
        "a phone has the same toolbar, whose New asks which workspace to add to"
    );
    fixture
        .test
        .snapshot("a_phone_lists_the_top_level_of_the_files");

    fixture.test.click(format!("file-tree.{folder}.chevron"));
    fixture.settle();
    assert!(
        fixture
            .host
            .watched_blocks()
            .contains(&BlockQuery::References(folder)),
        "the arrow side of a folder opens it where it is"
    );
    let mut roadmap = BlockInfo::new(inside, text_type, BlockParent::Block(folder));
    roadmap.set_intrinsic_name("Roadmap".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![roadmap]);
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{inside}.row")));
    assert!(
        !fixture.test.shown("file-tree.back"),
        "expanding a folder stays at the level it is on"
    );

    fixture.test.click(format!("file-tree.{folder}.row"));
    fixture.settle();
    assert!(
        fixture.opened().is_empty(),
        "a folder is zoomed into, never opened"
    );
    assert!(fixture.test.shown("file-tree.back"));
    assert!(
        !fixture.test.shown(&format!("file-tree.{notes}.row")),
        "zooming in shows only what the folder holds"
    );
    assert!(
        fixture.test.shown("file-tree.new"),
        "a folder the account may edit offers New"
    );
    fixture
        .test
        .snapshot("a_phone_shows_a_folder_it_walked_into");

    fixture.test.click(format!("file-tree.{inside}.row"));
    fixture.settle();
    assert_eq!(fixture.opened(), vec![inside], "tapping a file opens it");

    fixture.test.click("file-tree.back");
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{notes}.row")));
    assert!(
        fixture.test.shown("file-tree.back"),
        "back leaves for My files"
    );
    fixture.test.click("file-tree.back");
    fixture.settle();
    assert!(fixture.test.shown("file-tree.shared.row"));
    assert!(!fixture.test.shown("file-tree.back"));
}
