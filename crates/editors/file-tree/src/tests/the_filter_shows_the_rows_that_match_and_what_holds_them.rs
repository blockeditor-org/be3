use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Key;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn the_filter_shows_the_rows_that_match_and_what_holds_them() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let (folder, launch, retro, notes) = (
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        Uuid::from_u128(5),
    );
    let text = Uuid::from_u128(7);
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    projects.references = vec![launch, retro];
    let mut listed = BlockInfo::new(notes, text, BlockParent::Block(MY_FILES));
    listed.set_intrinsic_name("Notes".to_owned());
    my_files(&mut fixture, vec![projects, listed]);
    fixture.settle();
    fixture.test.click(format!("file-tree.{folder}.chevron"));
    fixture.settle();
    let mut plan = BlockInfo::new(launch, text, BlockParent::Block(folder));
    plan.set_intrinsic_name("Plan for launch".to_owned());
    let mut looked = BlockInfo::new(retro, text, BlockParent::Block(folder));
    looked.set_intrinsic_name("Retro".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![plan, looked]);
    fixture.settle();
    fixture.test.snapshot("the_files_toolbar_at_rest");

    fixture.test.click("file-tree.filter");
    fixture.settle();
    fixture.test.type_text("PLAN");
    fixture.settle();
    for shown in [MY_FILES, folder, launch] {
        assert!(
            fixture.test.shown(&format!("file-tree.{shown}.row")),
            "a match is shown with what holds it"
        );
    }
    for hidden in [retro, notes] {
        assert!(!fixture.test.shown(&format!("file-tree.{hidden}.row")));
    }
    assert!(fixture.says("1 match"));
    fixture.test.snapshot("the_filter_shows_a_match");

    fixture.test.type_text("zzz");
    fixture.settle();
    assert!(fixture.says("Nothing matches"));

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    for shown in [MY_FILES, folder, launch, retro, notes] {
        assert!(
            fixture.test.shown(&format!("file-tree.{shown}.row")),
            "Escape clears the filter and the rows stay open as they were"
        );
    }
    assert!(!fixture.test.shown("file-tree.matches"));
}
