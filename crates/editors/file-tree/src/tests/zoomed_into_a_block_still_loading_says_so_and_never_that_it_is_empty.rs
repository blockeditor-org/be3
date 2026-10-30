use std::time::Duration;

use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn zoomed_into_a_block_still_loading_says_so_and_never_that_it_is_empty() {
    let mut fixture = editor();
    let board = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(board, Uuid::nil(), BlockParent::Block(MY_FILES));
    listed.set_intrinsic_name("Board".to_owned());
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture
        .test
        .store()
        .leave_listing(BlockQuery::References(board));
    fixture.open_menu(board);
    fixture.click_item("Zoom in");
    assert_eq!(fixture.test.label("file-tree.here"), "Board");
    assert!(
        !fixture.says("Nothing here yet") && !fixture.says("Loading…"),
        "a block the host has not listed yet shows nothing at first, so a quick answer never flashes"
    );

    fixture.test.advance(Duration::from_millis(300));
    assert!(
        fixture.says("Loading…") && !fixture.says("Nothing here yet"),
        "a block still unlisted after a moment says it is loading, not that it holds nothing"
    );
    fixture
        .test
        .snapshot("zoomed_into_a_block_still_loading");

    fixture
        .host
        .fail_blocks(BlockQuery::References(board), "Offline".to_owned());
    fixture.settle();
    assert!(
        fixture.says("Offline") && !fixture.says("Nothing here yet"),
        "a block that could not be listed says why rather than that it holds nothing"
    );

    fixture
        .host
        .set_blocks(BlockQuery::References(board), Vec::new());
    fixture.settle();
    assert!(
        fixture.says("Nothing here yet") && !fixture.says("Loading…"),
        "once the host lists the block as holding nothing, the files say so"
    );
}
