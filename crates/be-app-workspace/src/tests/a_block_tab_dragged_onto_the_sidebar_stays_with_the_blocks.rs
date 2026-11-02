use super::*;

#[test]
fn a_block_tab_dragged_onto_the_sidebar_stays_with_the_blocks() {
    let (mut fixture, places) = two_blocks();

    fixture.drag_text("Minutes", SIDEBAR);

    assert_eq!(
        placed_in_sidebar(&fixture, places.minutes),
        None,
        "a block's tab is locked to the blocks area, so the sidebar does not take it"
    );
    fixture.click_text("Minutes");
    assert_eq!(
        placed_in_sidebar(&fixture, places.minutes),
        Some(false),
        "its tab is still in the blocks area"
    );
    fixture
        .test
        .snapshot("a_block_tab_dragged_onto_the_sidebar_stays_with_the_blocks");
}
