use std::time::Duration;

use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_folder_still_loading_says_so_only_after_a_moment() {
    let mut fixture = editor();
    let folder = Uuid::new_v4();
    let mut listed = BlockInfo::new(folder, Uuid::nil(), BlockParent::Block(MY_FILES));
    listed.references = vec![Uuid::new_v4()];
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let chevron = format!("file-tree.{folder}.chevron");

    fixture
        .test
        .store()
        .leave_listing(BlockQuery::References(folder));
    fixture.test.click(&chevron);
    fixture.settle();
    assert!(
        !fixture.says("Loading…") && !fixture.says("No references"),
        "a folder the host has not answered yet shows nothing at first, so a quick answer never flashes a loading row"
    );

    fixture.test.advance(Duration::from_millis(300));
    assert!(
        fixture.says("Loading…"),
        "a folder still unanswered after a moment says it is loading"
    );

    fixture
        .host
        .set_blocks(BlockQuery::References(folder), Vec::new());
    fixture.settle();
    assert!(
        fixture.says("No references") && !fixture.says("Loading…"),
        "once the host lists the folder as holding nothing, it says so"
    );
}
